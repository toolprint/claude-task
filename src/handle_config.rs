use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::Command;

use crate::config::Config;
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
    }

    Ok(())
}
