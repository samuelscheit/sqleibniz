use std::{fs, process::Command};

fn temp_sql(name: &str, content: &str) -> String {
    let path = std::env::temp_dir().join(format!(
        "sqleibniz-{name}-{}-{}.sql",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::write(&path, content).unwrap();
    path.into_os_string().into_string().unwrap()
}

fn sqleibniz(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_sqleibniz"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn sarif_success_stdout_is_parseable_json() {
    let file = temp_sql("valid", "VACUUM;");

    let output = sqleibniz(&["--sarif", "--ignore-config", &file]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["version"], "2.1.0");
    assert_eq!(json["runs"][0]["tool"]["driver"]["name"], "sqleibniz");
    assert_eq!(json["runs"][0]["results"].as_array().unwrap().len(), 0);
}

#[test]
fn sarif_diagnostic_uses_rule_message_and_location() {
    let file = temp_sql("invalid", "SELECT");

    let output = sqleibniz(&["--sarif", "--ignore-config", &file]);

    assert!(!output.status.success());
    assert!(output.stderr.is_empty());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let result = &json["runs"][0]["results"][0];
    assert_eq!(result["level"], "error");
    assert!(result["ruleId"].as_str().unwrap().len() > 0);
    assert!(result["message"]["text"].as_str().unwrap().len() > 0);
    assert_eq!(
        result["locations"][0]["physicalLocation"]["artifactLocation"]["uri"],
        file
    );
    assert!(
        result["locations"][0]["physicalLocation"]["region"]["startLine"]
            .as_u64()
            .unwrap()
            >= 1
    );
    assert!(
        result["locations"][0]["physicalLocation"]["region"]["startColumn"]
            .as_u64()
            .unwrap()
            >= 1
    );
}

#[test]
fn sarif_omits_disabled_rules() {
    let file = temp_sql("disabled", "");

    let output = sqleibniz(&["--sarif", "--ignore-config", "-D", "no-content", &file]);

    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["runs"][0]["results"].as_array().unwrap().len(), 0);
}

#[test]
fn sarif_conflicts_with_human_and_ast_modes() {
    for flag in ["--silent", "--kiss", "--ast", "--ast-json", "--lsp"] {
        let output = sqleibniz(&["--sarif", flag]);

        assert!(!output.status.success(), "{flag} should conflict");
        assert!(output.stdout.is_empty(), "{flag} should not write stdout");
    }
}

#[test]
fn sarif_missing_paths_and_unreadable_files_do_not_write_json() {
    let missing_paths = sqleibniz(&["--sarif"]);
    assert!(!missing_paths.status.success());
    assert!(missing_paths.stdout.is_empty());
    assert!(String::from_utf8_lossy(&missing_paths.stderr).contains("no source file"));

    let missing_file = sqleibniz(&["--sarif", "--ignore-config", "does-not-exist.sql"]);
    assert!(!missing_file.status.success());
    assert!(missing_file.stdout.is_empty());
    assert!(String::from_utf8_lossy(&missing_file.stderr).contains("failed to read file"));
}
