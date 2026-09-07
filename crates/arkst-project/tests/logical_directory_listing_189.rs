use arkst_project::{
    ResourceAccessError, ResourceEntryKind, VirtualPathBuf, VirtualProjectBuilder,
};

fn project() -> arkst_project::VirtualProject {
    VirtualProjectBuilder::new()
        .entry("docs/main.qd")
        .expect("valid entry")
        .add_source("docs/main.qd", "main")
        .expect("valid source")
        .add_source("docs/data/a.txt", "a")
        .expect("valid source")
        .add_source("docs/data/nested/b.txt", "b")
        .expect("valid source")
        .add_asset("docs/data/blob.bin", vec![0xff])
        .expect("valid asset")
        .add_asset("docs/data/nested/deep/c.bin", vec![0x01])
        .expect("valid asset")
        .build()
        .expect("valid project")
}

fn main_id(project: &arkst_project::VirtualProject) -> arkst_source::SourceId {
    project
        .sources()
        .get_id(&VirtualPathBuf::parse("docs/main.qd").unwrap())
        .unwrap()
}

#[test]
fn nonrecursive_listing_infers_only_nonempty_directories_and_is_path_ordered() {
    let project = project();
    let entries = project
        .list_resource_directory(main_id(&project), "data", false)
        .expect("logical directory exists");
    let observed = entries
        .iter()
        .map(|entry| (entry.name.as_str(), entry.kind))
        .collect::<Vec<_>>();
    assert_eq!(
        observed,
        vec![
            ("a.txt", ResourceEntryKind::File),
            ("blob.bin", ResourceEntryKind::File),
            ("nested", ResourceEntryKind::Directory),
        ]
    );
}

#[test]
fn recursive_listing_infers_intermediate_directories_and_keeps_binary_assets() {
    let project = project();
    let entries = project
        .list_resource_directory(main_id(&project), "data", true)
        .expect("logical directory exists");
    let observed = entries
        .iter()
        .map(|entry| (entry.path.as_str(), entry.kind))
        .collect::<Vec<_>>();
    assert_eq!(
        observed,
        vec![
            ("docs/data/a.txt", ResourceEntryKind::File),
            ("docs/data/blob.bin", ResourceEntryKind::File),
            ("docs/data/nested", ResourceEntryKind::Directory),
            ("docs/data/nested/b.txt", ResourceEntryKind::File),
            ("docs/data/nested/deep", ResourceEntryKind::Directory),
            ("docs/data/nested/deep/c.bin", ResourceEntryKind::File),
        ]
    );
}

#[test]
fn file_missing_escape_and_unrepresentable_empty_directory_fail_closed() {
    let project = project();
    let source = main_id(&project);
    assert!(matches!(
        project.list_resource_directory(source, "data/a.txt", false),
        Err(ResourceAccessError::NotDirectory(path)) if path.as_str() == "docs/data/a.txt"
    ));
    assert!(matches!(
        project.list_resource_directory(source, "empty", false),
        Err(ResourceAccessError::NotFound(path)) if path.as_str() == "docs/empty"
    ));
    assert!(matches!(
        project.list_resource_directory(source, "../../outside", false),
        Err(ResourceAccessError::Boundary(_))
    ));
    assert!(matches!(
        project.list_resource_directory(source, "/tmp", false),
        Err(ResourceAccessError::UnsupportedReference { .. })
    ));
}

#[test]
fn duplicate_source_asset_file_identity_is_deduplicated() {
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .unwrap()
        .add_source("main.qd", "main")
        .unwrap()
        .add_source("data/same.txt", "source")
        .unwrap()
        .add_asset("data/same.txt", vec![1, 2, 3])
        .unwrap()
        .build()
        .unwrap();
    let source = project
        .sources()
        .get_id(&VirtualPathBuf::parse("main.qd").unwrap())
        .unwrap();
    let entries = project
        .list_resource_directory(source, "data", false)
        .unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "same.txt");
    assert_eq!(entries[0].kind, ResourceEntryKind::File);
}

#[test]
fn impossible_file_directory_overlap_is_rejected_instead_of_guessed() {
    let project = VirtualProjectBuilder::new()
        .entry("main.qd")
        .unwrap()
        .add_source("main.qd", "main")
        .unwrap()
        .add_source("data/a", "file")
        .unwrap()
        .add_asset("data/a/child.bin", vec![1])
        .unwrap()
        .build()
        .unwrap();
    let source = project
        .sources()
        .get_id(&VirtualPathBuf::parse("main.qd").unwrap())
        .unwrap();
    assert!(matches!(
        project.list_resource_directory(source, "data", false),
        Err(ResourceAccessError::InconsistentDirectoryTree(path)) if path.as_str() == "data/a"
    ));
}

#[test]
fn current_directory_and_directory_only_filtering_are_explicit_189() {
    let project = VirtualProjectBuilder::new()
        .entry("docs/main.qd")
        .unwrap()
        .add_source("docs/main.qd", "main")
        .unwrap()
        .add_asset("docs/nested/only.bin", vec![1])
        .unwrap()
        .build()
        .unwrap();
    let source = project
        .sources()
        .get_id(&VirtualPathBuf::parse("docs/main.qd").unwrap())
        .unwrap();

    let current = project.list_resource_directory(source, ".", false).unwrap();
    let observed = current
        .iter()
        .map(|entry| (entry.name.as_str(), entry.kind))
        .collect::<Vec<_>>();
    assert_eq!(
        observed,
        vec![
            ("main.qd", ResourceEntryKind::File),
            ("nested", ResourceEntryKind::Directory),
        ]
    );

    let nested = project
        .list_resource_directory(source, "nested", false)
        .unwrap();
    assert_eq!(nested.len(), 1);
    assert_eq!(nested[0].name, "only.bin");
    assert_eq!(nested[0].kind, ResourceEntryKind::File);
}
