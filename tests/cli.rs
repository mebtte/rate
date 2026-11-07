use std::process::{Command, Output};

const SENTINEL: &str = "rate-test-private-value";

fn run(arguments: &[&str], basic_auth: Option<&str>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rate"));
    command.args(arguments).env_remove("RATE_BASIC_AUTH");
    if let Some(value) = basic_auth {
        command.env("RATE_BASIC_AUTH", value);
    }
    command.output().unwrap()
}

#[test]
fn basic_auth_help_hides_environment_credentials() {
    let output = run(&["--help"], Some(SENTINEL));
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).unwrap();
    assert!(help.contains("--basic-auth <USER:PASSWORD>"));
    assert!(help.contains("RATE_BASIC_AUTH"));
    assert!(!help.contains(SENTINEL));
}

#[test]
fn malformed_basic_auth_from_argument_and_environment_stays_private() {
    for output in [
        run(&["--basic-auth", SENTINEL], None),
        run(&["--basic-auth", &format!("--{SENTINEL}")], None),
        run(&[], Some(SENTINEL)),
    ] {
        assert!(!output.status.success());
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(error.contains("Basic auth requires USER:PASSWORD"));
        assert!(!error.contains(SENTINEL));
    }
}
