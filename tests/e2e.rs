use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_contract-verification"))
}

#[test]
fn verify_borrow_logic() {
    let output = bin()
        .args([
            "--standard-json-input",
            "tests/fixtures/BorrowLogic.input.json",
            "--contract-name",
            "BorrowLogic",
            "--solc-version",
            "0.8.27",
            "tests/fixtures/BorrowLogic.onchain.bytecode",
        ])
        .output()
        .expect("failed to execute binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "expected success, got exit code {:?}\nstdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(stdout.contains("verification succeeded"));
}
