use anyhow::Result;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

/// Helper to run claude-task with arguments and capture output
fn run_claude_task(args: &[&str]) -> Result<(String, String, bool)> {
    let exe_path = PathBuf::from(env!("CARGO_BIN_EXE_claude-task"));
    let output = Command::new(exe_path).args(args).output()?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    Ok((stdout, stderr, output.status.success()))
}

#[test]
fn test_config_init_creates_default_config() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("test-config.json");

    let (stdout, _stderr, success) = run_claude_task(&[
        "--config-path",
        config_path.to_str().unwrap(),
        "config",
        "init",
    ])?;

    assert!(success);
    assert!(stdout.contains("Created config file at:"));
    assert!(config_path.exists());

    // Verify config content
    let content = std::fs::read_to_string(&config_path)?;
    let config: serde_json::Value = serde_json::from_str(&content)?;
    assert_eq!(config["version"], "0.1.0");
    assert_eq!(config["paths"]["branchPrefix"], "claude-task/");

    Ok(())
}

#[test]
fn test_config_init_with_existing_file() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("test-config.json");

    // Create the config first
    std::fs::write(&config_path, "{}")?;

    let (stdout, _stderr, success) = run_claude_task(&[
        "--config-path",
        config_path.to_str().unwrap(),
        "config",
        "init",
    ])?;

    assert!(success);
    assert!(stdout.contains("Config file already exists"));
    assert!(stdout.contains("Use --force to overwrite"));

    Ok(())
}

#[test]
fn test_config_init_force_overwrites() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("test-config.json");

    // Create a config with different content
    std::fs::write(&config_path, r#"{"version": "old"}"#)?;

    let (stdout, _stderr, success) = run_claude_task(&[
        "--config-path",
        config_path.to_str().unwrap(),
        "config",
        "init",
        "--force",
    ])?;

    assert!(success);
    assert!(stdout.contains("Created config file at:"));

    // Verify it was overwritten with default
    let content = std::fs::read_to_string(&config_path)?;
    let config: serde_json::Value = serde_json::from_str(&content)?;
    assert_eq!(config["version"], "0.1.0");

    Ok(())
}

#[test]
fn test_config_validate_valid_config() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("test-config.json");

    // Create a valid config
    run_claude_task(&[
        "--config-path",
        config_path.to_str().unwrap(),
        "config",
        "init",
    ])?;

    let (stdout, _stderr, success) = run_claude_task(&[
        "--config-path",
        config_path.to_str().unwrap(),
        "config",
        "validate",
    ])?;

    assert!(success);
    assert!(stdout.contains("Config file is valid!"));
    assert!(stdout.contains("Resolved paths:"));

    Ok(())
}

#[test]
fn test_config_validate_invalid_config() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("test-config.json");

    // Create an invalid config (missing required fields)
    std::fs::write(&config_path, r#"{"version": ""}"#)?;

    let (stdout, stderr, success) = run_claude_task(&[
        "--config-path",
        config_path.to_str().unwrap(),
        "config",
        "validate",
    ])?;

    assert!(!success);
    // The error could be from parsing or validation
    assert!(
        stdout.contains("Config file validation failed")
            || stdout.contains("version cannot be empty")
            || stderr.contains("Failed to parse config file")
            || stderr.contains("missing field")
    );

    Ok(())
}

#[test]
fn test_config_show_json_format() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("test-config.json");

    // Create a config
    run_claude_task(&[
        "--config-path",
        config_path.to_str().unwrap(),
        "config",
        "init",
    ])?;

    let (stdout, _stderr, success) = run_claude_task(&[
        "--config-path",
        config_path.to_str().unwrap(),
        "config",
        "show",
        "--json",
    ])?;

    assert!(success);

    // Verify it's valid JSON
    let config: serde_json::Value = serde_json::from_str(&stdout)?;
    assert_eq!(config["version"], "0.1.0");

    Ok(())
}

#[test]
fn test_config_show_pretty_format() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("test-config.json");

    // Create a config
    run_claude_task(&[
        "--config-path",
        config_path.to_str().unwrap(),
        "config",
        "init",
    ])?;

    let (stdout, _stderr, success) = run_claude_task(&[
        "--config-path",
        config_path.to_str().unwrap(),
        "config",
        "show",
    ])?;

    assert!(success);
    assert!(stdout.contains("Claude Task Configuration"));
    assert!(stdout.contains("Version: 0.1.0"));
    assert!(stdout.contains("Worktree Base Dir:"));
    assert!(stdout.contains("Docker:"));

    Ok(())
}

#[test]
fn test_config_custom_path_not_found() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("nonexistent.json");

    let (_stdout, stderr, success) = run_claude_task(&[
        "--config-path",
        config_path.to_str().unwrap(),
        "config",
        "show",
    ])?;

    assert!(!success);
    assert!(stderr.contains("Config file not found at specified path"));

    Ok(())
}

#[test]
fn test_config_cli_args_override() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("test-config.json");

    // Create a config with custom values
    let custom_config = r#"{
        "version": "0.1.0",
        "paths": {
            "worktreeBaseDir": "~/custom-worktrees",
            "taskBaseHomeDir": "~/custom-home",
            "branchPrefix": "custom/"
        },
        "docker": {
            "imageName": "claude-task:dev",
            "volumePrefix": "claude-task-",
            "volumes": {
                "home": "claude-task-home",
                "npmCache": "claude-task-npm-cache",
                "nodeCache": "claude-task-node-cache"
            },
            "containerNamePrefix": "claude-task-",
            "defaultWebViewProxyPort": 4618,
            "defaultHtMcpPort": null,
            "environmentVariables": {}
        },
        "claudeUserConfig": {
            "configPath": "~/.claude.json",
            "userMemoryPath": "~/.claude/CLAUDE.md"
        },
        "worktree": {
            "defaultOpenCommand": null,
            "autoCleanOnRemove": false
        },
        "globalOptionDefaults": {
            "debug": true,
            "openEditorAfterCreate": false,
            "buildImageBeforeRun": false
        }
    }"#;

    std::fs::write(&config_path, custom_config)?;

    // Run with --debug flag which should override config value
    let (_stdout, _stderr, success) = run_claude_task(&[
        "--config-path",
        config_path.to_str().unwrap(),
        "--debug",
        "version",
    ])?;

    assert!(success);
    // The debug flag from CLI should be active even though config has debug: true
    // This test mainly verifies that the config loads successfully with CLI overrides

    Ok(())
}
