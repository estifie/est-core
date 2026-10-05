//! Golden vectors: the EST contract must print exactly what agents parse.
//! Shapes follow Angel's `docs/cli.md`, plus the versioned `"v":1` field.

use est_core::cli;

#[test]
fn envelope_ok_with_body() {
    assert_eq!(
        cli::ok("\"secrets\":26"),
        "{\"ok\":true,\"v\":1,\"secrets\":26}"
    );
}

#[test]
fn envelope_ok_bare() {
    assert_eq!(cli::ok(""), "{\"ok\":true,\"v\":1}");
}

#[test]
fn envelope_err_escapes_quotes() {
    assert_eq!(
        cli::err("no job \"x\""),
        "{\"ok\":false,\"v\":1,\"error\":\"no job \\\"x\\\"\"}"
    );
}

#[test]
fn exit_codes_match_contract() {
    assert_eq!(cli::ExitCode::Ok.code(), 0);
    assert_eq!(cli::ExitCode::Failed.code(), 1);
    assert_eq!(cli::ExitCode::Usage.code(), 2);
}
