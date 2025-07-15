use std::env;
use std::process::Command;

/// Test the MCP config functionality by running the CLI command
/// Equivalent to: cargo run -- run --mcp-config test/test.mcp.json --debug --yes "list mcp servers"
#[tokio::test]
#[ignore = "mcp"]
async fn test_mcp_config_validation() {
    // Get the project root directory
    let project_root = env::current_dir().expect("Failed to get current directory");

    // Path to the test MCP config file
    let mcp_config_path = project_root.join("tests").join("test.mcp.json");

    // Ensure the test MCP config file exists
    assert!(
        mcp_config_path.exists(),
        "Test MCP config file not found at: {}",
        mcp_config_path.display()
    );

    // Build the binary first
    let build_output = Command::new("cargo")
        .args(["build", "--bin", "claude-task"])
        .current_dir(&project_root)
        .output()
        .expect("Failed to build the binary");

    assert!(
        build_output.status.success(),
        "Build failed: {}",
        String::from_utf8_lossy(&build_output.stderr)
    );

    // Path to the built binary
    let binary_path = project_root
        .join("target")
        .join("debug")
        .join("claude-task");

    // Run the CLI command with MCP config, explicitly using Docker
    let output = Command::new(&binary_path)
        .args([
            "run",
            "--execution-env",
            "docker",
            "--mcp-config",
            mcp_config_path.to_str().unwrap(),
            "--debug",
            "--yes",
            "\"list mcp servers\"",
        ])
        .current_dir(&project_root)
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Print output for debugging
    println!("STDOUT:\n{stdout}");
    println!("STDERR:\n{stderr}");

    // Test that MCP config file validation passed
    assert!(
        stdout.contains("MCP config file found:") || stderr.contains("MCP config file found:"),
        "MCP config file validation should have passed. Output: {stdout}\nError: {stderr}"
    );

    // Test that the relative path was expanded to absolute path
    let expected_path = mcp_config_path.to_string_lossy().to_string();
    assert!(
        stdout.contains(&expected_path) || stderr.contains(&expected_path),
        "Should contain expanded absolute path: {expected_path}. Output: {stdout}\nError: {stderr}"
    );

    // Test that debug mode is enabled
    assert!(
        stdout.contains("🔍 Debug mode enabled") || stderr.contains("🔍 Debug mode enabled"),
        "Debug mode should be enabled. Output: {stdout}\nError: {stderr}"
    );

    // Test that --yes flag worked (skipped confirmation)
    assert!(
        stdout.contains("✓ Skipping confirmation") || stderr.contains("✓ Skipping confirmation"),
        "--yes flag should skip confirmation. Output: {stdout}\nError: {stderr}"
    );
}

/// Test MCP config validation with non-existent file
#[tokio::test]
#[ignore = "mcp"]
async fn test_mcp_config_file_not_found() {
    let project_root = env::current_dir().expect("Failed to get current directory");

    // Build the binary first
    let build_output = Command::new("cargo")
        .args(["build", "--bin", "claude-task"])
        .current_dir(&project_root)
        .output()
        .expect("Failed to build the binary");

    assert!(
        build_output.status.success(),
        "Build failed: {}",
        String::from_utf8_lossy(&build_output.stderr)
    );

    let binary_path = project_root
        .join("target")
        .join("debug")
        .join("claude-task");

    // Run with non-existent MCP config file, explicitly using Docker
    let output = Command::new(&binary_path)
        .args([
            "run",
            "--execution-env",
            "docker",
            "--mcp-config",
            "nonexistent.json",
            "--yes",
            "test prompt",
        ])
        .current_dir(&project_root)
        .output()
        .expect("Failed to execute command");

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should fail with error about file not found
    assert!(
        !output.status.success(),
        "Command should fail with non-existent MCP config file"
    );

    assert!(
        stderr.contains("MCP config file not found"),
        "Should show MCP config file not found error. Error: {stderr}"
    );
}

/// Test MCP config validation with relative path
#[tokio::test]
#[ignore = "mcp"]
async fn test_mcp_config_relative_path() {
    let project_root = env::current_dir().expect("Failed to get current directory");

    // Build the binary first
    let build_output = Command::new("cargo")
        .args(["build", "--bin", "claude-task"])
        .current_dir(&project_root)
        .output()
        .expect("Failed to build the binary");

    assert!(
        build_output.status.success(),
        "Build failed: {}",
        String::from_utf8_lossy(&build_output.stderr)
    );

    let binary_path = project_root
        .join("target")
        .join("debug")
        .join("claude-task");

    // Run with relative path to MCP config, explicitly using Docker
    let output = Command::new(&binary_path)
        .args([
            "run",
            "--execution-env",
            "docker",
            "--mcp-config",
            "tests/test.mcp.json", // relative path
            "--debug",
            "--yes",
            "\"test prompt\"",
        ])
        .current_dir(&project_root)
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should successfully find and validate the file
    assert!(
        stdout.contains("MCP config file found:") || stderr.contains("MCP config file found:"),
        "Should find MCP config file with relative path. Output: {stdout}\nError: {stderr}"
    );

    // Should show the expanded absolute path
    let expected_absolute = project_root
        .join("tests")
        .join("test.mcp.json")
        .to_string_lossy()
        .to_string();

    assert!(
        stdout.contains(&expected_absolute) || stderr.contains(&expected_absolute),
        "Should show expanded absolute path: {expected_absolute}. Output: {stdout}\nError: {stderr}"
    );
}
