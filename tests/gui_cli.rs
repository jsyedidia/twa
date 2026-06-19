#[cfg(feature = "gui")]
#[test]
fn gui_smoke_test_launches_and_exits() {
    let exe = env!("CARGO_BIN_EXE_gui");
    let output = std::process::Command::new(exe)
        .arg("--smoke-test")
        .output()
        .expect("failed to run gui binary");

    assert!(
        output.status.success(),
        "gui smoke test failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("gui smoke test ok"),
        "stdout:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
}
