#[test]
fn platform_neutral_env_path_has_no_ambient_process_lookup() {
    let sources = [
        (
            "arkst-engine/lib.rs",
            include_str!("../../arkst-engine/src/lib.rs"),
        ),
        (
            "arkst-engine/evaluator.rs",
            include_str!("../../arkst-engine/src/evaluator.rs"),
        ),
        ("arkst-core/lib.rs", include_str!("../src/lib.rs")),
    ];

    let forbidden = [
        "std::env::var(",
        "std::env::var_os(",
        "std::env::vars(",
        "std::env::vars_os(",
        "std::env::current_dir(",
        "std::env::set_var(",
        "std::env::remove_var(",
    ];

    for (path, source) in sources {
        for pattern in forbidden {
            assert!(
                !source.contains(pattern),
                "{path} must not use ambient process state via {pattern}"
            );
        }
    }
}

#[test]
fn default_core_compile_path_does_not_inject_environment_authority() {
    let source = include_str!("../src/lib.rs");

    assert!(
        source.contains(
            "compile_with_inputs(\n        project,\n        options,\n        Capabilities::compatibility_default(),\n        None,"
        ),
        "ordinary compile() must remain fail-closed for .env"
    );
}
