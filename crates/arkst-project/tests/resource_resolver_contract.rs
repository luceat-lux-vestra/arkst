use arkst_project::{
    ResourceAccessError, ResourceResolutionBase, ResourceRoot, VirtualPathBuf, VirtualProjectBuilder,
};

fn project() -> arkst_project::VirtualProject {
    VirtualProjectBuilder::new()
        .entry("docs/main.qd")
        .expect("valid entry")
        .add_source("docs/main.qd", "main")
        .expect("valid source")
        .add_source("docs/partials/a.qd", "partial")
        .expect("valid source")
        .add_source("subdocuments/root.qd", "subdocument")
        .expect("valid source")
        .add_source("utils/example.qd", "utility")
        .expect("valid source")
        .build()
        .expect("valid project")
}

#[test]
fn public_resolver_has_explicit_bases_and_canonical_identity() {
    let project = project();
    let main = project
        .sources()
        .get_id(&VirtualPathBuf::parse("docs/main.qd").unwrap())
        .unwrap();
    let target = VirtualPathBuf::parse("docs/partials/a.qd").unwrap();
    let target_id = project.sources().get_id(&target).unwrap();

    for reference in ["partials/a.qd", "./partials//a.qd", "tmp/../partials/a.qd"] {
        let resolved = project
            .resolve_logical_resource(ResourceResolutionBase::Source(main), reference)
            .unwrap();
        assert_eq!(resolved, target);
        assert_eq!(project.sources().get_id(&resolved), Some(target_id));
    }

    assert_eq!(
        project
            .resolve_logical_resource(ResourceResolutionBase::ProjectRoot, "docs/partials/a.qd")
            .unwrap(),
        target
    );
}

#[test]
fn public_resolver_fails_closed_and_projects_logical_roots_only() {
    let project = project();
    let main = project
        .sources()
        .get_id(&VirtualPathBuf::parse("docs/main.qd").unwrap())
        .unwrap();
    let subdocument = project
        .sources()
        .get_id(&VirtualPathBuf::parse("subdocuments/root.qd").unwrap())
        .unwrap();
    let utility = project
        .sources()
        .get_id(&VirtualPathBuf::parse("utils/example.qd").unwrap())
        .unwrap();

    for reference in ["/etc/passwd", r"C:\secret.txt", "https://example.test/a"] {
        assert!(matches!(
            project.resolve_logical_resource(ResourceResolutionBase::Source(main), reference),
            Err(ResourceAccessError::UnsupportedReference { .. })
        ));
    }
    assert!(matches!(
        project.resolve_logical_resource(
            ResourceResolutionBase::Source(main),
            "../../outside.txt",
        ),
        Err(ResourceAccessError::Boundary(_))
    ));

    assert_eq!(
        project
            .relative_path_to_resource_root(utility, ResourceRoot::Project)
            .unwrap(),
        ".."
    );
    assert_eq!(
        project
            .relative_path_to_resource_root(utility, ResourceRoot::Source(subdocument))
            .unwrap(),
        "../subdocuments"
    );
}
