use std::fs;
use std::process::{Command, Output};
use tempfile::tempdir;

fn run(args: &[&std::ffi::OsStr]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_arkst"));
    command.args(args);
    command.output().expect("arkst process must run")
}

fn os(value: &str) -> &std::ffi::OsStr {
    std::ffi::OsStr::new(value)
}

#[test]
fn build_check_and_inspect_share_explicit_library_ingestion() {
    let dir = tempdir().unwrap();
    let project = dir.path().join("project");
    let libs = dir.path().join("libs");
    fs::create_dir_all(&project).unwrap();
    fs::create_dir_all(&libs).unwrap();
    let input = project.join("main.qd");
    fs::write(&input, ".include {reader}\n").unwrap();
    fs::write(libs.join("reader.qd"), "native library payload\n").unwrap();

    let build = run(&[
        os("build"),
        input.as_os_str(),
        os("--libs"),
        libs.as_os_str(),
    ]);
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let typst = fs::read_to_string(project.join("main.typ")).unwrap();
    assert!(typst.contains("native library payload"), "{typst}");

    let check = run(&[os("check"), input.as_os_str(), os("-l"), libs.as_os_str()]);
    assert!(
        check.status.success(),
        "{}",
        String::from_utf8_lossy(&check.stderr)
    );

    let inspect = run(&[
        os("inspect"),
        input.as_os_str(),
        os("--libs"),
        libs.as_os_str(),
        os("--emit"),
        os("typst"),
    ]);
    assert!(
        inspect.status.success(),
        "{}",
        String::from_utf8_lossy(&inspect.stderr)
    );
    assert!(String::from_utf8_lossy(&inspect.stdout).contains("native library payload"));
}

#[test]
fn discovery_is_non_recursive_lowercase_only_and_eager_for_selected_files() {
    let dir = tempdir().unwrap();
    let project = dir.path().join("project");
    let libs = dir.path().join("libs");
    let nested = libs.join("nested");
    fs::create_dir_all(&project).unwrap();
    fs::create_dir_all(&nested).unwrap();
    let input = project.join("main.qd");
    fs::write(&input, ".include {good}\n").unwrap();
    fs::write(libs.join("good.qd"), "selected library\n").unwrap();
    fs::write(libs.join("ignored.QD"), [0xff, 0xfe]).unwrap();
    fs::write(nested.join("nested.qd"), [0xff, 0xfe]).unwrap();

    let ok = run(&[
        os("check"),
        input.as_os_str(),
        os("--libs"),
        libs.as_os_str(),
    ]);
    assert!(
        ok.status.success(),
        "{}",
        String::from_utf8_lossy(&ok.stderr)
    );

    fs::write(libs.join("unused-but-selected.qd"), [0xff, 0xfe]).unwrap();
    let eager = run(&[
        os("check"),
        input.as_os_str(),
        os("--libs"),
        libs.as_os_str(),
    ]);
    assert!(!eager.status.success());
    let stderr = String::from_utf8_lossy(&eager.stderr);
    assert!(stderr.contains("cannot read loadable library"), "{stderr}");
    assert!(stderr.contains("UTF-8"), "{stderr}");
}

#[test]
fn invalid_library_directory_inputs_fail_before_evaluation() {
    let dir = tempdir().unwrap();
    let project = dir.path().join("project");
    fs::create_dir_all(&project).unwrap();
    let input = project.join("main.qd");
    fs::write(&input, "plain text\n").unwrap();

    let missing = dir.path().join("missing-libs");
    let missing_result = run(&[
        os("check"),
        input.as_os_str(),
        os("--libs"),
        missing.as_os_str(),
    ]);
    assert!(!missing_result.status.success());
    assert!(String::from_utf8_lossy(&missing_result.stderr)
        .contains("cannot resolve loadable library directory"));

    let not_directory = dir.path().join("not-a-directory");
    fs::write(&not_directory, "x").unwrap();
    let file_result = run(&[
        os("check"),
        input.as_os_str(),
        os("--libs"),
        not_directory.as_os_str(),
    ]);
    assert!(!file_result.status.success());
    assert!(String::from_utf8_lossy(&file_result.stderr)
        .contains("loadable library path is not a directory"));
}

#[test]
#[cfg(unix)]
fn selected_library_symlink_cannot_escape_explicit_directory() {
    use std::os::unix::fs::symlink;

    let dir = tempdir().unwrap();
    let project = dir.path().join("project");
    let libs = dir.path().join("libs");
    fs::create_dir_all(&project).unwrap();
    fs::create_dir_all(&libs).unwrap();
    let input = project.join("main.qd");
    fs::write(&input, ".include {escape}\n").unwrap();
    let outside = dir.path().join("outside.qd");
    fs::write(&outside, "outside library\n").unwrap();
    symlink(&outside, libs.join("escape.qd")).unwrap();

    let result = run(&[
        os("check"),
        input.as_os_str(),
        os("--libs"),
        libs.as_os_str(),
    ]);
    assert!(!result.status.success());
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.contains("symlink escape"), "{stderr}");
    assert!(stderr.contains("outside library root"), "{stderr}");
}
