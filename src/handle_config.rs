use anyhow::{Context, Result};
use dialoguer::Select;
use std::path::PathBuf;
use std::process::Command;

use crate::config::{Config, ExecutionEnvironment};
use crate::ConfigCommands;

pub async fn handle_config_command(
    command: ConfigCommands,
    config_path: Option<&PathBuf>,
) -> Result<()> {
    match command {
        ConfigCommands::Init { force } => {
            let path = config_path
                .cloned()
                .unwrap_or_else(Config::default_config_path);

            if path.exists() && !force {
                println!("⚠️  Config file already exists at: {}", path.display());
                println!("   Use --force to overwrite");
                return Ok(());
            }

            let default_config = Config::default();
            default_config.save(&path)?;
            println!("✅ Created config file at: {}", path.display());

            // Show a sample of the config
            println!("\nSample configuration:");
            println!("{}", serde_json::to_string_pretty(&default_config)?);
        }
        ConfigCommands::Edit => {
            let path = config_path
                .cloned()
                .unwrap_or_else(Config::default_config_path);

            if !path.exists() {
                println!("⚠️  Config file not found at: {}", path.display());
                println!("   Run 'ct config init' to create a default config");
                return Ok(());
            }

            // Try to open with $EDITOR, falling back to common editors
            let editor = std::env::var("EDITOR").unwrap_or_else(|_| {
                if Command::new("code").arg("--version").output().is_ok() {
                    "code".to_string()
                } else if Command::new("vim").arg("--version").output().is_ok() {
                    "vim".to_string()
                } else if Command::new("nano").arg("--version").output().is_ok() {
                    "nano".to_string()
                } else {
                    "vi".to_string()
                }
            });

            println!("📝 Opening config in {editor}...");
            let status = Command::new(&editor)
                .arg(&path)
                .status()
                .with_context(|| format!("Failed to open editor: {editor}"))?;

            if !status.success() {
                return Err(anyhow::anyhow!("Editor exited with error"));
            }

            // Validate after editing
            match Config::load(Some(&path)) {
                Ok(_) => println!("✅ Config file is valid"),
                Err(e) => {
                    println!("⚠️  Warning: Config file has errors:");
                    println!("   {e}");
                }
            }
        }
        ConfigCommands::Show { json } => {
            let config = Config::load(config_path)?;

            if json {
                println!("{}", serde_json::to_string_pretty(&config)?);
            } else {
                // Pretty print with sections
                println!("Claude Task Configuration");
                println!("========================");
                println!();
                println!("Version: {}", config.version);
                println!();
                println!("Paths:");
                println!("  Worktree Base Dir: {}", config.paths.worktree_base_dir);
                println!("  Task Base Home Dir: {}", config.paths.task_base_home_dir);
                println!("  Branch Prefix: {}", config.paths.branch_prefix);
                println!();
                println!("Docker:");
                println!("  Image Name: {}", config.docker.image_name);
                println!("  Volume Prefix: {}", config.docker.volume_prefix);
                println!(
                    "  Container Name Prefix: {}",
                    config.docker.container_name_prefix
                );
                if let Some(port) = config.docker.default_web_view_proxy_port {
                    println!("  Default Web View Proxy Port: {port}");
                } else {
                    println!("  Default Web View Proxy Port: disabled");
                }
                if let Some(port) = config.docker.default_ht_mcp_port {
                    println!("  Default HT-MCP Port: {port}");
                }
                println!();
                println!("Claude User Config:");
                println!("  Config Path: {}", config.claude_user_config.config_path);
                println!(
                    "  User Memory Path: {}",
                    config.claude_user_config.user_memory_path
                );
                println!();
                println!("Worktree:");
                if let Some(cmd) = &config.worktree.default_open_command {
                    println!("  Default Open Command: {cmd}");
                }
                println!(
                    "  Auto Clean on Remove: {}",
                    config.worktree.auto_clean_on_remove
                );
                println!();
                println!("Global Option Defaults:");
                println!("  Debug: {}", config.global_option_defaults.debug);
                println!(
                    "  Open Editor After Create: {}",
                    config.global_option_defaults.open_editor_after_create
                );
                println!(
                    "  Build Image Before Run: {}",
                    config.global_option_defaults.build_image_before_run
                );
                println!(
                    "  Require HT-MCP: {}",
                    config.global_option_defaults.require_ht_mcp
                );
                println!();
                println!("Task Runner: {:?}", config.task_runner);

                // Show Claude credentials status (masked)
                println!();
                println!("Claude Credentials:");
                if let Some(credentials) = &config.claude_credentials {
                    let masked_token = if credentials.token.len() > 8 {
                        format!(
                            "{}...{}",
                            &credentials.token[..4],
                            &credentials.token[credentials.token.len() - 4..]
                        )
                    } else {
                        "****".to_string()
                    };
                    println!("  Token: {masked_token} (configured)");
                } else {
                    println!("  Token: <not configured>");
                }

                // Show Kubernetes config if present
                if let Some(kube_config) = &config.kube_config {
                    println!();
                    println!("Kubernetes Configuration:");
                    println!(
                        "  Context: {}",
                        kube_config
                            .context
                            .as_ref()
                            .unwrap_or(&"<auto-detect>".to_string())
                    );
                    println!(
                        "  Namespace: {}",
                        kube_config
                            .namespace
                            .as_ref()
                            .unwrap_or(&"<auto-generate>".to_string())
                    );
                    println!("  Image: {}", kube_config.image);
                    println!("  Git Secret Name: {}", kube_config.git_secret_name);
                    println!("  Git Secret Key: {}", kube_config.git_secret_key);
                    if let Some(pull_secret) = &kube_config.image_pull_secret {
                        println!("  Image Pull Secret: {pull_secret}");
                    }
                    println!("  Namespace Confirmed: {}", kube_config.namespace_confirmed);
                } else {
                    println!();
                    println!("Kubernetes Configuration: <not configured>");
                }
            }
        }
        ConfigCommands::Validate => {
            let path = config_path
                .cloned()
                .unwrap_or_else(Config::default_config_path);

            println!("🔍 Validating config file at: {}", path.display());

            match Config::load(Some(&path)) {
                Ok(config) => {
                    // Additional validation beyond basic loading
                    let expanded_worktree = Config::expand_tilde(&config.paths.worktree_base_dir);
                    let expanded_task_home = Config::expand_tilde(&config.paths.task_base_home_dir);

                    println!("✅ Config file is valid!");
                    println!();
                    println!("Resolved paths:");
                    println!("  Worktree Base Dir: {}", expanded_worktree.display());
                    println!("  Task Base Home Dir: {}", expanded_task_home.display());
                }
                Err(e) => {
                    println!("❌ Config file validation failed:");
                    println!("   {e}");
                    return Err(e);
                }
            }
        }
        ConfigCommands::Runner { runner } => {
            let path = config_path
                .cloned()
                .unwrap_or_else(Config::default_config_path);

            let mut config = Config::load(Some(&path))?;

            // If runner not specified, show interactive selection
            let new_runner = if let Some(r) = runner {
                r
            } else {
                println!("Select task runner:");
                let options = vec!["Docker", "Kubernetes"];
                let current_idx = match config.task_runner {
                    ExecutionEnvironment::Docker => 0,
                    ExecutionEnvironment::Kubernetes => 1,
                };

                let selection = Select::new()
                    .items(&options)
                    .default(current_idx)
                    .interact()?;

                match selection {
                    0 => ExecutionEnvironment::Docker,
                    1 => ExecutionEnvironment::Kubernetes,
                    _ => unreachable!(),
                }
            };

            // Update config
            let old_runner = config.task_runner.clone();
            config.task_runner = new_runner.clone();

            // Save config
            config.save(&path)?;

            println!("✅ Task runner updated: {old_runner:?} → {new_runner:?}");

            // Show additional setup instructions if switching to Kubernetes
            if new_runner == ExecutionEnvironment::Kubernetes
                && old_runner != ExecutionEnvironment::Kubernetes
            {
                println!();
                println!("ℹ️  You've switched to Kubernetes mode. Next steps:");
                println!("   1. Run: claude setup kubernetes");
                println!("   2. Ensure you have GITHUB_TOKEN set or gh CLI authenticated");
                println!("   3. Run tasks with: claude run \"your task\"");
            }
        }
        ConfigCommands::Token => {
            let path = config_path
                .cloned()
                .unwrap_or_else(Config::default_config_path);

            let mut config = Config::load(Some(&path))?;

            // Prompt for token using password input
            use dialoguer::Password;

            println!("📝 Set Claude OAuth Token");
            println!();
            println!("Please paste your long-lived token from 'claude setup-token'.");
            println!("The token will be hidden as you type/paste.");
            println!();

            let token = Password::new().with_prompt("Token").interact()?;

            if token.is_empty() {
                println!("❌ Token cannot be empty");
                return Ok(());
            }

            // Update config with token
            config.claude_credentials = Some(crate::config::ClaudeCredentials { token });

            // Save config
            config.save(&path)?;

            println!();
            println!("✅ Claude OAuth token saved successfully!");
            println!();
            println!("Your claude-task setup can now use this token for authentication.");
            println!("The token will be injected as CLAUDE_CODE_OAUTH_TOKEN in containers.");
        }
    }

    Ok(())
}
