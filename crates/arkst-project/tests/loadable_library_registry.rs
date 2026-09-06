use arkst_project::{BuildError, VirtualPathBuf, VirtualProjectBuilder};

fn project_with_libraries(order: &[&str]) -> arkst_project::VirtualProject {
    let mut builder = VirtualProjectBuilder::new()
        .entry("docs/main.qd")
        .expect("entry")
        .add_source("docs/main.qd", "main")
        .expect("main")
        .add_source("docs/nested.qd", "nested")
        .expect("nested");
    for name in order {
        builder = builder.add_loadable_library(*name, format!("source-{name}"));
    }
    builder.build().expect("project")
}

#[test]
fn registry_is_case_sensitive_and_does_not_change_project_source_ids() {
    let baseline = project_with_libraries(&[]);
    let project = project_with_libraries(&["zeta", "alpha"]);
    for path in ["docs/main.qd", "docs/nested.qd"] {
        let path = VirtualPathBuf::parse(path).unwrap();
        assert_eq!(
            baseline.sources().get_id(&path),
            project.sources().get_id(&path)
        );
    }
    let alpha = project.loadable_library("alpha").expect("alpha");
    let zeta = project.loadable_library("zeta").expect("zeta");
    assert_eq!(alpha.source(), "source-alpha");
    assert!(project.loadable_library("Alpha").is_none());
    assert!(alpha.source_id().0 < zeta.source_id().0);
    assert_eq!(
        project.loadable_library_name_by_source_id(alpha.source_id()),
        Some("alpha")
    );
    assert!(project.sources().get_by_id(alpha.source_id()).is_none());
}

#[test]
fn registry_rejects_empty_and_duplicate_names_atomically() {
    let empty = VirtualProjectBuilder::new()
        .entry("main.qd")
        .unwrap()
        .add_source("main.qd", "main")
        .unwrap()
        .add_loadable_library("", "x")
        .build();
    assert!(matches!(empty, Err(BuildError::EmptyLoadableLibraryName)));

    let duplicate = VirtualProjectBuilder::new()
        .entry("main.qd")
        .unwrap()
        .add_source("main.qd", "main")
        .unwrap()
        .add_loadable_library("hello", "one")
        .add_loadable_library("hello", "two")
        .build();
    assert!(
        matches!(duplicate, Err(BuildError::DuplicateLoadableLibrary(name)) if name == "hello")
    );
}
