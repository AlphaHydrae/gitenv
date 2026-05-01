use std::process::Command;

#[test]
fn prints_hello_world_from_cli() {
    let output = Command::new(env!("CARGO_BIN_EXE_gitenv"))
        .output()
        .expect("binary should run");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "Hello, World!\n");
    assert!(output.stderr.is_empty());
}
