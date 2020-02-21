// build on https://rust-lang-nursery.github.io/cli-wg/tutorial/testing.html#testing-cli-applications-by-running-them
use assert_cmd::prelude::*; // Add methods on commands
                            // use predicates::prelude::*;
use std::process::Command; // Run programs // Used for writing assertions

fn test_program(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::main_binary()?;
    cmd.arg(path);
    cmd.env("RUST_BACKTRACE", "1");
    cmd.env("RUST_LOG", "trace");
    // has to point to the toolchain declared in ``rust-toolchain`` file
    cmd.env(
        "LD_LIBRARY_PATH",
        "/home/tom/.rustup/toolchains/nightly-2020-01-07-x86_64-unknown-linux-gnu/lib",
    );

    let result = cmd.assert().success();
    // run 'cargo test -- --nocapture' to see the actual output
    let output = result.get_output();
    if output.status.success() {
        println!("{}", String::from_utf8_lossy(&output.stdout));
    };
    Ok(())
}

#[test]
fn minimal_program_test() {
    test_program("tests/sample_programs/minimal_program.rs").unwrap();
}

#[test]
fn minimal_nondeadlock_test() {
    test_program("tests/sample_programs/minimal_nondeadlock.rs").unwrap();
}

#[test]
fn minimal_deadlock_test() {
    test_program("tests/sample_programs/minimal_deadlock.rs").unwrap();
}

#[test]
fn function_call_test() {
    test_program("tests/sample_programs/function_call.rs").unwrap();
}
