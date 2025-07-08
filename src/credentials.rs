use super::assets;
use anyhow::{Context, Result};
use keyring::Entry;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

#[derive(Serialize, Deserialize, Debug)]
pub struct OAuthAccount {
    #[serde(rename = "accountUuid")]
    pub account_uuid: String,
    #[serde(rename = "emailAddress")]
    pub email_address: String,
    #[serde(rename = "organizationUuid")]
    pub organization_uuid: String,
    #[serde(rename = "organizationRole")]
    pub organization_role: String,
    #[serde(rename = "workspaceRole")]
    pub workspace_role: Option<String>,
    #[serde(rename = "organizationName")]
    pub organization_name: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ClaudeConfig {
    #[serde(rename = "oauthAccount")]
    pub oauth_account: Option<OAuthAccount>,
    #[serde(rename = "userID")]
    pub user_id: Option<String>,
    #[serde(rename = "hasCompletedOnboarding")]
    pub has_completed_onboarding: Option<bool>,
    #[serde(rename = "mcpServers")]
    pub mcp_servers: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Serialize, Deserialize, Debug)]
struct FullClaudeConfig {
    #[serde(rename = "oauthAccount")]
    oauth_account: Option<OAuthAccount>,
    #[serde(rename = "userID")]
    user_id: Option<String>,
    #[serde(rename = "hasCompletedOnboarding")]
    has_completed_onboarding: Option<bool>,
    #[serde(rename = "mcpServers")]
    mcp_servers: Option<HashMap<String, serde_json::Value>>,
    #[serde(flatten)]
    other: HashMap<String, serde_json::Value>,
}

fn get_current_username() -> Result<String> {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .context("Could not determine current username")
}

pub async fn extract_keychain_credentials() -> Result<String> {
    let username = get_current_username()?;

    // First request biometric authentication on macOS
    #[cfg(target_os = "macos")]
    {
        if let Err(e) = request_biometric_authentication().await {
            println!("⚠️  Biometric authentication failed: {e}");
            println!("   Falling back to keychain access without biometrics");
        }
    }

    let entry = Entry::new("Claude Code-credentials", &username)
        .context("Failed to create keychain entry")?;

    entry
        .get_password()
        .context("Failed to retrieve password from keychain")
}

#[cfg(target_os = "macos")]
async fn request_biometric_authentication() -> Result<()> {
    use localauthentication_rs::{LAPolicy, LocalAuthentication};

    let local_auth = LocalAuthentication::new();

    // Check if biometric authentication is available
    let policy = LAPolicy::DeviceOwnerAuthenticationWithBiometrics;

    if !local_auth.can_evaluate_policy(policy) {
        return Err(anyhow::anyhow!(
            "Biometric authentication not available on this device"
        ));
    }

    println!("🔐 Requesting biometric authentication (Touch ID/Face ID)...");

    // Request biometric authentication
    let success =
        local_auth.evaluate_policy(policy, "Claude Code needs to access your credentials");

    if success {
        println!("✓ Biometric authentication successful");
        Ok(())
    } else {
        Err(anyhow::anyhow!("Biometric authentication failed"))
    }
}

#[cfg(not(target_os = "macos"))]
async fn request_biometric_authentication() -> Result<()> {
    // Not implemented for other platforms
    Ok(())
}

pub fn read_and_filter_claude_config() -> Result<ClaudeConfig> {
    let home_dir = std::env::var("HOME").context("Could not find HOME directory")?;
    let claude_config_path = format!("{home_dir}/.claude.json");

    let content = fs::read_to_string(&claude_config_path)
        .with_context(|| format!("Failed to read {claude_config_path}"))?;

    let full_config: FullClaudeConfig =
        serde_json::from_str(&content).context("Failed to parse claude config JSON")?;

    Ok(ClaudeConfig {
        oauth_account: full_config.oauth_account,
        user_id: full_config.user_id,
        has_completed_onboarding: full_config.has_completed_onboarding,
        mcp_servers: full_config.mcp_servers,
    })
}

// Note: MCP configuration is now handled dynamically in the container
// using 'claude mcp add-json' commands instead of static config files

pub async fn setup_credentials_and_config(task_base_home_dir: &str, debug: bool) -> Result<()> {
    println!("Setting up Claude configuration...");

    // Expand home directory if needed
    let base_dir = if task_base_home_dir.starts_with('~') {
        let home_dir = std::env::var("HOME").context("Could not find HOME directory")?;
        task_base_home_dir.replacen('~', &home_dir, 1)
    } else {
        task_base_home_dir.to_string()
    };

    // Create output directories
    let claude_dir = format!("{base_dir}/.claude");
    fs::create_dir_all(&claude_dir)
        .with_context(|| format!("Failed to create directory: {claude_dir}"))?;

    // Extract keychain credentials with biometric authentication
    println!("Extracting keychain credentials...");
    let credentials = extract_keychain_credentials()
        .await
        .context("Failed to extract keychain credentials")?;

    // Write credentials to file in .claude directory
    let credentials_path = format!("{claude_dir}/.credentials.json");
    fs::write(&credentials_path, credentials).context("Failed to write credentials file")?;

    println!("✓ Keychain credentials extracted to {credentials_path}");

    // Read and filter claude config
    println!("Reading and filtering claude config...");
    let filtered_config =
        read_and_filter_claude_config().context("Failed to read and filter claude config")?;

    // Write filtered config to base directory (not inside .claude folder)
    let filtered_json = serde_json::to_string_pretty(&filtered_config)
        .context("Failed to serialize filtered config")?;

    let config_path = format!("{base_dir}/.claude.json");
    fs::write(&config_path, filtered_json).context("Failed to write filtered config file")?;

    println!("✓ Filtered claude config written to {config_path}");

    // Write the CLAUDE.md file to the claude directory
    let claude_md_path = format!("{claude_dir}/CLAUDE.md");
    let claude_md_content = assets::get_claude_md_content();
    fs::write(&claude_md_path, claude_md_content)
        .with_context(|| format!("Failed to write CLAUDE.md to {claude_md_path}"))?;

    println!("✓ CLAUDE.md written to {claude_md_path}");

    // Note: MCP configuration is now handled dynamically in the container
    // using 'claude mcp add-json' commands instead of static files
    println!("✓ MCP servers will be configured dynamically in the container");

    // Create Docker volume with bind mount to the setup directory
    println!("Creating Docker volume 'claude-task-home'...");
    create_docker_home_volume(&base_dir).await?;

    // Debug: Display volume contents if debug mode is enabled
    if debug {
        println!("\n🔍 Debug: Inspecting volume contents...");
        inspect_docker_volume_contents().await?;
    }

    println!("Setup complete!");

    Ok(())
}

async fn create_docker_home_volume(base_dir: &str) -> Result<()> {
    use std::process::Command;

    // First, try to remove existing volume if it exists
    let _ = Command::new("docker")
        .args(["volume", "rm", "claude-task-home"])
        .output();

    // Create the Docker volume with bind mount
    let output = Command::new("docker")
        .args([
            "volume",
            "create",
            "--driver",
            "local",
            "--opt",
            "type=bind",
            "--opt",
            &format!("device={base_dir}"),
            "--opt",
            "o=bind,ro", // Read-only bind mount
            "--label",
            "project=claude-task",
            "claude-task-home",
        ])
        .output()
        .context("Failed to execute docker volume create command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("Docker volume create failed: {}", stderr));
    }

    println!("✓ Docker volume 'claude-task-home' created with read-only bind mount to {base_dir}");

    Ok(())
}

async fn inspect_docker_volume_contents() -> Result<()> {
    use std::process::Command;

    // Run a temporary container to inspect the volume contents
    let output = Command::new("docker")
        .args([
            "run",
            "--rm",
            "-v",
            "claude-task-home:/inspect",
            "alpine",
            "find",
            "/inspect",
            "-type",
            "f",
            "-exec",
            "ls",
            "-la",
            "{}",
            ";",
        ])
        .output()
        .context("Failed to execute docker run command for volume inspection")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!(
            "Docker volume inspection failed: {}",
            stderr
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    println!("📁 Volume contents:");
    println!("{stdout}");

    // Also show directory structure
    let tree_output = Command::new("docker")
        .args([
            "run",
            "--rm",
            "-v",
            "claude-task-home:/inspect",
            "alpine",
            "sh",
            "-c",
            "find /inspect -type d | sort",
        ])
        .output()
        .context("Failed to execute docker run command for directory tree")?;

    if tree_output.status.success() {
        let tree_stdout = String::from_utf8_lossy(&tree_output.stdout);
        println!("\n📂 Directory structure:");
        for line in tree_stdout.lines() {
            if let Some(path) = line.strip_prefix("/inspect") {
                if path.is_empty() {
                    println!("└── /");
                } else {
                    let depth = path.matches('/').count();
                    let indent = "  ".repeat(depth);
                    let name = path.split('/').next_back().unwrap_or(path);
                    println!("{indent}├── {name}");
                }
            }
        }
    }

    Ok(())
}
