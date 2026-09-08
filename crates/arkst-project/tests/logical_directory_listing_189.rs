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
        .add_directory("docs/empty")
        .expect("valid empty directory")
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
fn file_missing_escape_and_unregistered_directory_fail_closed() {
    let project = project();
    let source = main_id(&project);
    assert!(matches!(
        project.list_resource_directory(source, "data/a.txt", false),
        Err(ResourceAccessError::NotDirectory(path)) if path.as_str() == "docs/data/a.txt"
    ));
    assert!(project
        .list_resource_directory(source, "empty", false)
        .expect("explicit empty directory exists")
        .is_empty());
    assert!(matches!(
        project.list_resource_directory(source, "missing-empty", false),
        Err(ResourceAccessError::NotFound(path)) if path.as_str() == "docs/missing-empty"
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
fn explicit_empty_directories_participate_in_metadata_parent_and_recursive_listing_189() {
    let project = VirtualProjectBuilder::new()
        .entry("docs/main.qd")
        .unwrap()
        .add_source("docs/main.qd", "main")
        .unwrap()
        .add_directory("docs/data/empty/deep")
        .unwrap()
        .build()
        .unwrap();
    let source = main_id(&project);

    assert_eq!(
        project
            .resolve_existing_resource_path(source, "data")
            .unwrap()
            .as_str(),
        "docs/data"
    );
    assert_eq!(
        project
            .resolve_existing_resource_path(source, "data/empty/deep")
            .unwrap()
            .as_str(),
        "docs/data/empty/deep"
    );
    let direct = project
        .list_resource_directory(source, "data", false)
        .unwrap();
    assert_eq!(direct.len(), 1);
    assert_eq!(direct[0].path.as_str(), "docs/data/empty");
    assert_eq!(direct[0].kind, ResourceEntryKind::Directory);

    let recursive = project
        .list_resource_directory(source, "data", true)
        .unwrap();
    let observed = recursive
        .iter()
        .map(|entry| (entry.path.as_str(), entry.kind))
        .collect::<Vec<_>>();
    assert_eq!(
        observed,
        vec![
            ("docs/data/empty", ResourceEntryKind::Directory),
            ("docs/data/empty/deep", ResourceEntryKind::Directory),
        ]
    );
    assert!(project
        .list_resource_directory(source, "data/empty/deep", false)
        .unwrap()
        .is_empty());
}

#[test]
fn explicit_directory_builder_rejects_root_duplicates_and_file_ancestor_conflicts_189() {
    assert!(VirtualProjectBuilder::new().add_directory("").is_err());

    let duplicate = VirtualProjectBuilder::new()
        .entry("main.qd")
        .unwrap()
        .add_source("main.qd", "main")
        .unwrap()
        .add_directory("empty")
        .unwrap()
        .add_directory("empty")
        .unwrap()
        .build();
    assert!(matches!(
        duplicate,
        Err(arkst_project::BuildError::DuplicateDirectory(path)) if path.as_str() == "empty"
    ));

    let exact_conflict = VirtualProjectBuilder::new()
        .entry("main.qd")
        .unwrap()
        .add_source("main.qd", "main")
        .unwrap()
        .add_asset("data", vec![1])
        .unwrap()
        .add_directory("data")
        .unwrap()
        .build();
    assert!(matches!(
        exact_conflict,
        Err(arkst_project::BuildError::FileDirectoryConflict(path)) if path.as_str() == "data"
    ));

    let ancestor_conflict = VirtualProjectBuilder::new()
        .entry("main.qd")
        .unwrap()
        .add_source("main.qd", "main")
        .unwrap()
        .add_asset("data", vec![1])
        .unwrap()
        .add_directory("data/empty")
        .unwrap()
        .build();
    assert!(matches!(
        ancestor_conflict,
        Err(arkst_project::BuildError::FileDirectoryConflict(path)) if path.as_str() == "data"
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
