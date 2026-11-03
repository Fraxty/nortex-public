use std::fs;
use std::process::Command;

#[test]
fn check_command_accepts_valid_source_and_reports_invalid_source() {
    let temp_dir = std::env::temp_dir().join(format!("nortex-cli-check-{}", std::process::id()));
    fs::create_dir_all(&temp_dir).expect("temporary test directory should be created");
    let valid_source = temp_dir.join("valid.nx");
    let invalid_source = temp_dir.join("invalid.nx");
    fs::write(&valid_source, "public int main() { return 42; }")
        .expect("valid source should be written");
    fs::write(&invalid_source, "public int main( { return 0; }")
        .expect("invalid source should be written");

    let valid = Command::new(env!("CARGO_BIN_EXE_nortexc"))
        .arg("check")
        .arg(&valid_source)
        .output()
        .expect("nortexc check should start for valid source");
    assert!(
        valid.status.success(),
        "valid check failed: {}",
        String::from_utf8_lossy(&valid.stderr)
    );

    let invalid = Command::new(env!("CARGO_BIN_EXE_nortexc"))
        .arg("check")
        .arg(&invalid_source)
        .output()
        .expect("nortexc check should start for invalid source");
    assert!(!invalid.status.success(), "invalid syntax must fail check");
    let stderr = String::from_utf8_lossy(&invalid.stderr);
    assert!(
        stderr.contains("error[NX"),
        "unexpected diagnostics: {stderr}"
    );
    assert!(
        stderr.contains("invalid.nx"),
        "missing source location: {stderr}"
    );

    fs::remove_dir_all(&temp_dir).expect("temporary test files should be removed");
}
