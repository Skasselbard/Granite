// build on https://rust-lang-nursery.github.io/cli-wg/tutorial/testing.html#testing-cli-applications-by-running-them
use assert_cmd::prelude::*; // Add methods on commands
use predicates::prelude::*;
use std::process::{Command, Stdio}; // Run programs // Used for writing assertions
#[test]
fn minimal_program_test() -> Result<(), Box<std::error::Error>> {
    let mut cmd = Command::main_binary()?;
    cmd.arg("tests/sample_programs/minimal_program.rs");
    cmd.env("RUST_LOG", "trace");
    let result = cmd.assert().success();
    // run 'cargo test -- --nocapture' to see the actual output
    let output = result.get_output();
    println!("{}", String::from_utf8_lossy(&output.stdout));
    Ok(())
}
