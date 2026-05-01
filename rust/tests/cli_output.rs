use std::process::Command;

#[test]
fn show_the_default_message_on_stdout() {
    let output = Command::new(env!("CARGO_BIN_EXE_gitenv"))
        .output()
        .expect("binary should run");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "Hello, World!\n");
    assert!(output.stderr.is_empty());
}
