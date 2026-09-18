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

    let forbidden = ["std::env", "use std::{env", "use std::{ env"];

    for (path, source) in sources {
        for pattern in forbidden {
            assert!(
                !source.contains(pattern),
                "{path} must not use ambient process state via {pattern}"
            );
        }
    }
}
