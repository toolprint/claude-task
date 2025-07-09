use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand};
use dialoguer::Select;
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

// Include the generated MCP help text
include!(concat!(env!("OUT_DIR"), "/mcp_help.rs"));

mod assets;
mod config;
mod credentials;
mod docker;
mod mcp;
pub mod permission;

use permission::ApprovalToolPermission;

#[derive(Debug)]
struct TaskRunConfig<'a> {
    prompt: &'a str,
    task_id: Option<String>,
    build: bool,
    workspace_dir: Option<Option<String>>,
    approval_tool_permission: Option<String>,
    debug: bool,
    mcp_config: Option<String>,
    skip_confirmation: bool,
    worktree_base_dir: &'a str,
    task_base_home_dir: &'a str,
    open_editor: bool,
    ht_mcp_port: Option<u16>,
    web_view_proxy_port: u16,
}

use credentials::setup_credentials_and_config;
use docker::{ClaudeTaskConfig, DockerManager};

#[derive(Subcommand)]
enum WorktreeCommands {
    /// Create a new git worktree
    #[command(visible_alias = "c")]
    Create {
        /// Task ID for the worktree
        task_id: String,
    },
    /// List current git worktrees
    #[command(visible_alias = "l")]
    List,
    /// Remove and clean up a worktree
    #[command(visible_alias = "rm")]
    Remove {
        /// Task ID to remove (will be prefixed with branch_prefix)
        task_id: String,
    },
    /// Open a worktree in your IDE
    #[command(visible_alias = "o")]
    Open,
    /// Clean up all claude-task git worktrees
    #[command(visible_alias = "cl")]
    Clean {
        /// Skip confirmation prompt
        #[arg(long, short = 'y')]
        yes: bool,
        /// Force removal of worktrees even if they have uncommitted changes
        #[arg(long, short = 'f')]
        force: bool,
    },
}

#[derive(Subcommand)]
enum DockerCommands {
    /// Initialize shared docker volumes for Claude tasks
    #[command(visible_alias = "i")]
    Init {
        /// Refresh credentials by running setup first
        #[arg(long)]
        refresh_credentials: bool,
    },
    /// List Docker volumes for Claude tasks
    #[command(visible_alias = "l")]
    List,
    /// Clean up all shared Docker volumes
    #[command(visible_alias = "c")]
    Clean,
}

#[derive(Parser)]
#[command(name = "claude-task")]
#[command(about = "Claude Task Management CLI")]
struct Cli {
    /// Base directory for worktrees
    #[arg(long, global = true, default_value = "~/.claude-task/worktrees")]
    worktree_base_dir: String,

    /// Branch prefix for worktrees
    #[arg(short = 'b', long, global = true, default_value = "claude-task/")]
    branch_prefix: String,

    /// Base directory for task home directory and setup files
    #[arg(long, global = true, default_value = "~/.claude-task/home")]
    task_base_home_dir: String,

    /// Enable debug mode
    #[arg(short = 'd', long, global = true)]
    debug: bool,

    /// Path to the configuration file (defaults to ~/.claude-task/config.json)
    #[arg(long, global = true, value_name = "PATH", help = "Path to config file")]
    config_path: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Setup claude-task with your current environment
    #[command(visible_alias = "s")]
    Setup,
    /// Git worktree management commands
    #[command(visible_alias = "wt")]
    Worktree {
        #[command(subcommand)]
        command: WorktreeCommands,
    },
    /// Docker management commands
    #[command(visible_alias = "d")]
    Docker {
        #[command(subcommand)]
        command: DockerCommands,
    },
    /// Run a Claude task in a local docker container
    #[command(visible_alias = "r")]
    Run {
        /// The prompt to pass to Claude
        prompt: String,
        /// Optional task ID (generates short ID if not provided)
        #[arg(long)]
        task_id: Option<String>,
        /// Build the image before running (default: false)
        #[arg(long)]
        build: bool,
        /// Custom workspace directory to mount (overrides worktree creation). If provided without value, uses current directory
        #[arg(long, value_name = "DIR")]
        workspace_dir: Option<Option<String>>,
        /// Claude Code permission statement to pass for approval tool. Example: "mcp__approval_server__tool_name"
        #[arg(short = 'a', long, value_name = "PERMISSION_STATEMENT")]
        approval_tool_permission: Option<String>,
        /// Optional MCP config file path that will be mounted to the container and passed to Claude
        #[arg(short = 'c', long, value_name = "MCP_CONFIG_FILEPATH")]
        mcp_config: Option<String>,
        /// Skip confirmation prompts (automatically answer yes)
        #[arg(long, short)]
        yes: bool,
        /// Open IDE in worktree after task creation
        #[arg(short = 'e', long)]
        open_editor: bool,
        /// Port to expose for HT-MCP web interface (e.g., 8080)
        #[arg(long)]
        ht_mcp_port: Option<u16>,
        /// Port to expose for web view proxy to see terminal commands the task runs (default: 4618)
        #[arg(long, default_value = "4618")]
        web_view_proxy_port: u16,
    },
    /// Clean up both claude-task git worktrees and docker volumes
    #[command(visible_alias = "c")]
    Clean {
        /// Skip confirmation prompt
        #[arg(long, short = 'y')]
        yes: bool,
        /// Force removal of worktrees even if they have uncommitted changes
        #[arg(long, short = 'f')]
        force: bool,
    },
    /// Launch MCP server on stdio
    #[command(after_help = MCP_HELP_TEXT)]
    Mcp,
    /// Print version information
    #[command(visible_alias = "v")]
    Version,
}

fn sanitize_branch_name(name: &str) -> String {
    let re = Regex::new(r"[^a-zA-Z0-9\-_]").unwrap();
    re.replace_all(name, "-").to_string()
}

fn find_git_repo_root(start_path: &Path) -> Result<PathBuf> {
    let mut current = start_path;
    loop {
        if current.join(".git").exists() {
            return Ok(current.to_path_buf());
        }
        match current.parent() {
            Some(parent) => current = parent,
            None => return Err(anyhow::anyhow!("No git repository found")),
        }
    }
}

fn get_repo_name(worktree_path: &Path) -> String {
    // Try to get repo name from git remote
    // Git handles worktrees automatically, so we can just run from the worktree path
    if let Ok(output) = Command::new("git")
        .args(["config", "--get", "remote.origin.url"])
        .current_dir(worktree_path)
        .output()
    {
        if output.status.success() {
            let url = String::from_utf8_lossy(&output.stdout).trim().to_string();

            // Extract org/repo from URL
            // Handle HTTPS format: https://github.com/org/repo.git
            if url.starts_with("https://") || url.starts_with("http://") {
                // Remove protocol and domain
                let path_part = url
                    .split("://")
                    .nth(1)
                    .and_then(|s| s.split('/').skip(1).collect::<Vec<_>>().join("/").into());

                if let Some(path) = path_part {
                    let clean_path = path.strip_suffix(".git").unwrap_or(&path);
                    if clean_path.contains('/') {
                        return clean_path.to_string();
                    }
                }
            }

            // Handle SSH format: git@github.com:org/repo.git
            if let Some(repo_part) = url.split(':').nth(1) {
                let clean_path = repo_part.strip_suffix(".git").unwrap_or(repo_part);
                if clean_path.contains('/') {
                    return clean_path.to_string();
                }
            }
        }
    }

    // Fallback to directory name
    worktree_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown")
        .to_string()
}

fn get_worktree_directory(worktree_base_dir: &str) -> Result<PathBuf> {
    let worktree_dir = worktree_base_dir.to_string();

    let worktree_path = if worktree_dir.starts_with('/') || worktree_dir.starts_with('~') {
        // Absolute path or home directory path
        if worktree_dir.starts_with('~') {
            let home_dir = std::env::var("HOME").context("Could not find HOME directory")?;
            PathBuf::from(worktree_dir.replacen('~', &home_dir, 1))
        } else {
            PathBuf::from(worktree_dir)
        }
    } else {
        // Relative path - relative to current directory
        std::env::current_dir()
            .context("Could not get current directory")?
            .join(worktree_dir)
    };

    fs::create_dir_all(&worktree_path)
        .with_context(|| format!("Failed to create worktree directory: {worktree_path:?}"))?;
    Ok(worktree_path)
}

fn create_git_worktree(
    task_id: &str,
    branch_prefix: &str,
    worktree_base_dir: &str,
) -> Result<(PathBuf, String)> {
    let current_dir = std::env::current_dir().context("Could not get current directory")?;
    let repo_root = find_git_repo_root(&current_dir)?;

    let sanitized_name = sanitize_branch_name(task_id);
    let branch_name = format!("{branch_prefix}{sanitized_name}");

    let worktree_base_dir = get_worktree_directory(worktree_base_dir)?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let worktree_path = worktree_base_dir.join(format!("{sanitized_name}_{timestamp:x}"));

    println!("Creating git worktree...");
    println!("Repository root: {repo_root:?}");
    println!("Branch name: {branch_name}");
    println!("Worktree path: {worktree_path:?}");

    // Create the worktree
    let output = Command::new("git")
        .args(["worktree", "add", "-b", &branch_name])
        .arg(&worktree_path)
        .current_dir(&repo_root)
        .output()
        .context("Failed to execute git worktree command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("Git worktree command failed: {}", stderr));
    }

    println!("✓ Git worktree created successfully");
    println!("  Branch: {branch_name}");
    println!("  Path: {worktree_path:?}");

    Ok((worktree_path, branch_name))
}

fn list_git_worktrees(branch_prefix: &str) -> Result<()> {
    let current_dir = std::env::current_dir().context("Could not get current directory")?;
    let repo_root = find_git_repo_root(&current_dir)?;

    println!("Listing git worktrees with branch prefix '{branch_prefix}'...");
    println!("Repository root: {repo_root:?}");
    println!();

    let output = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(&repo_root)
        .output()
        .context("Failed to execute git worktree list command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!(
            "Git worktree list command failed: {}",
            stderr
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();

    if lines.is_empty() {
        println!("No worktrees found.");
        return Ok(());
    }

    let mut current_worktree: Option<(String, String, String)> = None; // (path, head, branch)
    let mut matching_worktrees = Vec::new();

    for line in lines {
        if line.starts_with("worktree ") {
            // If we have a previous worktree, check if it matches and store it
            if let Some((path, head, branch)) = current_worktree.take() {
                if should_include_worktree(&branch, branch_prefix, &path, &repo_root) {
                    matching_worktrees.push((path, head, branch));
                }
            }

            // Start new worktree
            let path = line.strip_prefix("worktree ").unwrap_or(line);
            current_worktree = Some((path.to_string(), String::new(), String::new()));
        } else if line.starts_with("HEAD ") {
            if let Some((_, ref mut head, _)) = current_worktree.as_mut() {
                let new_head = line.strip_prefix("HEAD ").unwrap_or(line);
                *head = new_head.to_string();
            }
        } else if line.starts_with("branch ") {
            if let Some((_, _, ref mut branch)) = current_worktree.as_mut() {
                let new_branch = line.strip_prefix("branch ").unwrap_or(line);
                *branch = new_branch.to_string();
            }
        } else if line == "bare" {
            if let Some((_, _, ref mut branch)) = current_worktree.as_mut() {
                *branch = "(bare)".to_string();
            }
        } else if line == "detached" {
            if let Some((_, _, ref mut branch)) = current_worktree.as_mut() {
                *branch = "(detached)".to_string();
            }
        }
    }

    // Handle the last worktree if it exists
    if let Some((path, head, branch)) = current_worktree {
        if should_include_worktree(&branch, branch_prefix, &path, &repo_root) {
            matching_worktrees.push((path, head, branch));
        }
    }

    // Print all matching worktrees
    if matching_worktrees.is_empty() {
        println!("No worktrees found matching branch prefix '{branch_prefix}'.");
    } else {
        for (path, head, branch) in matching_worktrees {
            print_worktree_info(&path, &head, &branch);
        }
    }

    Ok(())
}

fn should_include_worktree(
    branch: &str,
    branch_prefix: &str,
    path: &str,
    repo_root: &std::path::Path,
) -> bool {
    // Clean up branch name by removing refs/heads/ prefix for comparison
    let clean_branch = if branch.starts_with("refs/heads/") {
        branch.strip_prefix("refs/heads/").unwrap_or(branch)
    } else {
        branch
    };

    // Exclude the main repository directory (where .git folder is located)
    let worktree_path = std::path::Path::new(path);
    if worktree_path == repo_root {
        return false;
    }

    // Only include branches that start with the prefix (exclude main/master unless they're actual worktrees)
    clean_branch.starts_with(branch_prefix)
        || branch == "(bare)"
        || branch == "(detached)"
        // Include main/master only if they are actual worktrees (not the main repo)
        || ((clean_branch == "main" || clean_branch == "master") && worktree_path != repo_root)
}

fn print_worktree_info(path: &str, head: &str, branch: &str) {
    let path_buf = PathBuf::from(path);
    let dir_name = path_buf
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown");

    // Clean up branch name by removing refs/heads/ prefix
    let clean_branch = if branch.starts_with("refs/heads/") {
        branch.strip_prefix("refs/heads/").unwrap_or(branch)
    } else if branch.is_empty() {
        "unknown"
    } else {
        branch
    };

    // Determine if this is a Claude task worktree
    let is_claude_task = clean_branch.starts_with("claude-task/");
    let icon = if is_claude_task { "🌿" } else { "📁" };
    let type_label = if is_claude_task {
        " (Claude task)"
    } else {
        " (worktree)"
    };

    // Get repository name
    let repo_name = get_repo_name(&path_buf);

    // Check worktree status
    println!("{icon} {dir_name}{type_label}");
    println!("   Path: {path}");
    println!("   Repository: {repo_name}");
    println!("   Branch: {clean_branch}");
    println!(
        "   HEAD: {}",
        if head.len() > 7 { &head[..7] } else { head }
    );

    match check_worktree_status(&path_buf) {
        Ok(status) => {
            let status_icon = status.get_status_icon();
            let details = status.get_status_details();

            if status.is_clean() {
                if status.is_likely_merged {
                    let merge_type = status.merge_info.as_deref().unwrap_or("merged");
                    println!("   Status: {status_icon} Clean ({merge_type})");
                } else {
                    println!("   Status: {status_icon} Clean");
                }
            } else {
                println!("   Status: {status_icon} Unclean: {}", details.join(", "));

                // Show merge info if detected
                if status.is_likely_merged {
                    if let Some(ref info) = status.merge_info {
                        println!(
                            "   Note: Branch appears to be {info} - remote may have been deleted"
                        );
                    }
                }

                // Show changed files if any
                if !status.changed_files.is_empty() {
                    println!("   Changed files:");
                    for file in &status.changed_files {
                        println!("     - {file}");
                    }
                }

                // Show untracked files if any
                if !status.untracked_files.is_empty() {
                    println!("   Untracked files:");
                    for file in &status.untracked_files {
                        println!("     - {file}");
                    }
                }

                // Show unpushed commits if any
                if !status.unpushed_commits.is_empty() && !status.is_likely_merged {
                    println!("   Unpushed commits:");
                    for (commit_id, message) in &status.unpushed_commits {
                        println!("     - {commit_id} {message}");
                    }
                } else if !status.unpushed_commits.is_empty() && status.is_likely_merged {
                    println!("   Commits (likely already merged):");
                    for (commit_id, message) in &status.unpushed_commits {
                        println!("     - {commit_id} {message}");
                    }
                }
            }
        }
        Err(_) => {
            println!("   Status: ❓ Status unknown");
        }
    };

    println!();
}

fn remove_git_worktree(task_id: &str, branch_prefix: &str) -> Result<()> {
    let current_dir = std::env::current_dir().context("Could not get current directory")?;
    let repo_root = find_git_repo_root(&current_dir)?;

    let sanitized_id = sanitize_branch_name(task_id);
    let branch_name = format!("{branch_prefix}{sanitized_id}");

    println!("Removing git worktree for task '{task_id}'...");
    println!("Repository root: {repo_root:?}");
    println!("Target branch: {branch_name}");
    println!();

    // First, get list of worktrees to find the one with matching branch
    let output = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(&repo_root)
        .output()
        .context("Failed to execute git worktree list command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!(
            "Git worktree list command failed: {}",
            stderr
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();

    let mut worktree_path: Option<String> = None;
    let mut current_path: Option<String> = None;

    for line in lines {
        if line.starts_with("worktree ") {
            current_path = Some(line.strip_prefix("worktree ").unwrap_or(line).to_string());
        } else if line.starts_with("branch ") {
            let branch = line.strip_prefix("branch ").unwrap_or(line);
            let clean_branch = if branch.starts_with("refs/heads/") {
                branch.strip_prefix("refs/heads/").unwrap_or(branch)
            } else {
                branch
            };

            if clean_branch == branch_name {
                worktree_path = current_path.clone();
                break;
            }
        }
    }

    let worktree_path = match worktree_path {
        Some(path) => path,
        None => {
            println!("❌ No worktree found for branch '{branch_name}'");
            return Ok(());
        }
    };

    println!("Found worktree: {worktree_path}");

    // Remove the worktree
    println!("Removing worktree...");
    let output = Command::new("git")
        .args(["worktree", "remove", &worktree_path, "--force"])
        .current_dir(&repo_root)
        .output()
        .context("Failed to execute git worktree remove command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!(
            "Git worktree remove command failed: {}",
            stderr
        ));
    }

    println!("✓ Worktree removed: {worktree_path}");

    // Delete the branch
    println!("Deleting branch '{branch_name}'...");
    let output = Command::new("git")
        .args(["branch", "-D", &branch_name])
        .current_dir(&repo_root)
        .output()
        .context("Failed to execute git branch delete command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!("⚠️  Warning: Failed to delete branch '{branch_name}': {stderr}");
        println!("   You may need to delete it manually with: git branch -D {branch_name}");
    } else {
        println!("✓ Branch deleted: {branch_name}");
    }

    println!();
    println!("✅ Cleanup complete for task '{task_id}'");

    Ok(())
}

async fn init_shared_volumes(
    refresh_credentials: bool,
    task_base_home_dir: &str,
    debug: bool,
) -> Result<()> {
    println!("Initializing shared Docker volumes for Claude tasks...");
    if debug {
        println!("🔍 Refresh credentials: {refresh_credentials}");
        println!("🔍 Task base home dir: {task_base_home_dir}");
    }
    println!();

    // Create Docker manager
    let docker_manager = DockerManager::new().context("Failed to create Docker manager")?;

    // Create cache volumes (npm and node)
    println!("Creating cache volumes...");
    let dummy_config = ClaudeTaskConfig::default();
    docker_manager.create_volumes(&dummy_config).await?;

    // Run setup if requested
    if refresh_credentials {
        println!("Refreshing credentials...");
        setup_credentials_and_config(task_base_home_dir, debug).await?;
    }

    println!();
    println!("✅ All shared volumes are ready:");
    println!("   - claude-task-home (credentials and config)");
    println!("   - claude-task-npm-cache (shared npm cache)");
    println!("   - claude-task-node-cache (shared node cache)");

    Ok(())
}

fn generate_short_id() -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();

    let mut hasher = DefaultHasher::new();
    timestamp.hash(&mut hasher);

    // Get a short hash and format as hex
    format!("{:x}", hasher.finish())[..8].to_string()
}

fn open_ide_in_path(path: &str, ide: &str) -> Result<()> {
    println!("🚀 Opening {ide} in {path}...");

    let output = Command::new(ide)
        .arg(path)
        .output()
        .with_context(|| format!("Failed to execute {ide} command"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!(
            "Failed to open {} with {}: {}",
            path,
            ide,
            stderr
        ));
    }

    println!("✓ {ide} opened successfully");
    Ok(())
}

fn select_worktree_interactively(branch_prefix: &str) -> Result<()> {
    println!("🌿 Finding available worktrees...");
    let worktrees = get_matching_worktrees(branch_prefix)?;

    if worktrees.is_empty() {
        println!("No claude-task worktrees found matching prefix '{branch_prefix}'.");
        println!("Create a new worktree with: ct worktree create <task-id>");
        return Ok(());
    }

    // Filter to only claude-task worktrees and create display options
    let claude_worktrees: Vec<_> = worktrees
        .into_iter()
        .filter(|(_, _, branch)| {
            let clean_branch = if branch.starts_with("refs/heads/") {
                branch.strip_prefix("refs/heads/").unwrap_or(branch)
            } else {
                branch
            };
            clean_branch.starts_with(branch_prefix)
        })
        .collect();

    if claude_worktrees.is_empty() {
        println!("No claude-task worktrees found.");
        println!("Create a new worktree with: ct worktree create <task-id>");
        return Ok(());
    }

    // Create display options for the menu
    let mut options = Vec::new();
    for (path, _head, branch) in &claude_worktrees {
        let clean_branch = if branch.starts_with("refs/heads/") {
            branch.strip_prefix("refs/heads/").unwrap_or(branch)
        } else {
            branch
        };

        let path_buf = PathBuf::from(path);
        let dir_name = path_buf
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown");

        let display_name = format!("{dir_name} ({clean_branch})");
        options.push(display_name);
    }

    let selection = Select::new()
        .with_prompt("Select a worktree to open")
        .default(0)
        .items(&options)
        .interact()
        .context("Failed to get selection")?;

    let selected_worktree = &claude_worktrees[selection];
    let worktree_path = &selected_worktree.0;

    println!("Selected: {}", options[selection]);
    open_ide_in_path(worktree_path, "cursor")?;

    Ok(())
}

async fn run_claude_task(config: TaskRunConfig<'_>) -> Result<()> {
    if config.debug {
        println!("🔍 Debug mode enabled");
        println!("📝 Task parameters:");
        println!("   - Prompt: {}", config.prompt);
        println!("   - Task ID: {:?}", config.task_id);
        println!("   - Build: {}", config.build);
        println!("   - Workspace dir: {:?}", config.workspace_dir);
        println!(
            "   - Approval tool permission: {:?}",
            config.approval_tool_permission
        );
        println!("   - MCP config: {:?}", config.mcp_config);
        println!("   - Worktree base dir: {}", config.worktree_base_dir);
        println!("   - Task base home dir: {}", config.task_base_home_dir);
        println!();
    }

    let current_dir = std::env::current_dir().context("Could not get current directory")?;

    // Validate MCP config file if provided
    let validated_mcp_config = if let Some(mcp_path) = config.mcp_config {
        let mcp_config_path = if Path::new(&mcp_path).is_absolute() {
            PathBuf::from(mcp_path)
        } else {
            current_dir.join(mcp_path)
        };

        if !mcp_config_path.exists() {
            return Err(anyhow::anyhow!(
                "MCP config file not found: {}\nPlease ensure the file exists at the specified path.", 
                mcp_config_path.display()
            ));
        }

        if config.debug {
            println!("🔍 MCP config file found: {}", mcp_config_path.display());
        }

        Some(mcp_config_path.to_string_lossy().to_string())
    } else {
        None
    };

    // Handle approval tool permission configuration FIRST, before any setup
    let (permission_tool_arg, skip_permissions) = match config.approval_tool_permission {
        Some(tool) => (tool, false),
        None => {
            // Show warning and request confirmation
            println!("⚠️  WARNING: No approval tool permission specified!");
            println!("   This will run Claude with --dangerously-skip-permissions");
            println!("   Claude will have unrestricted access to execute commands without user approval.");

            // Extra warning if HT-MCP is enabled
            if config.ht_mcp_port.is_some() {
                println!();
                println!("🚨 ADDITIONAL WARNING: HT-MCP mode is enabled!");
                println!("   Skipping permissions defeats the purpose of HT-MCP integration.");
                println!("   Claude will be able to use built-in tools instead of HT-MCP,");
                println!("   making the web interface monitoring ineffective.");
                println!("   Consider providing an approval tool permission instead.");
            }

            println!();
            println!("   This is DANGEROUS and should only be used in trusted environments.");
            println!();

            if !config.skip_confirmation {
                print!("❓ Are you sure you want to proceed without permission prompts? [y/N]: ");
                use std::io::{self, Write};
                io::stdout().flush().context("Failed to flush stdout")?;

                let mut input = String::new();
                io::stdin()
                    .read_line(&mut input)
                    .context("Failed to read input")?;

                let input = input.trim().to_lowercase();
                if input != "y" && input != "yes" {
                    println!("❌ Task cancelled for safety.");
                    return Ok(());
                }
            } else {
                println!("✓ Skipping confirmation (--yes flag provided)");
            }

            println!("⚠️  Proceeding with dangerous permissions disabled...");
            println!();

            (String::new(), true)
        }
    };

    // Validate approval tool permission format if not empty
    if !permission_tool_arg.is_empty() {
        if let Err(e) = ApprovalToolPermission::parse(&permission_tool_arg) {
            return Err(anyhow::anyhow!(
                "Invalid approval tool permission format: {}\n\nExpected format: mcp__<server_name>__<tool_name>\nExample: mcp__approval_server__approve_command", 
                e
            ));
        }

        if config.debug {
            println!("✓ Approval tool permission format validated: {permission_tool_arg}");
        }
    }

    // Generate or use provided task ID
    let task_id = match config.task_id {
        Some(id) => id,
        None => generate_short_id(),
    };

    println!("Running Claude task with ID: {task_id}");
    println!("Prompt: {}", config.prompt);
    println!();

    // Determine workspace directory
    let workspace_path = match config.workspace_dir {
        Some(Some(custom_dir)) => {
            // Use custom directory provided
            let custom_path = PathBuf::from(&custom_dir);
            if !custom_path.exists() {
                return Err(anyhow::anyhow!(
                    "Custom workspace directory does not exist: {}",
                    custom_dir
                ));
            }
            println!("📁 Using custom workspace directory: {custom_dir}");
            custom_dir
        }
        Some(None) => {
            // --workspace-dir provided without value, use current directory
            println!("📁 Using current directory as workspace");
            current_dir.to_string_lossy().to_string()
        }
        None => {
            // Default: Create worktree
            println!("🌿 Creating git worktree for task...");
            let (worktree_path, branch_name) =
                create_git_worktree(&task_id, "claude-task/", config.worktree_base_dir)?;
            println!("✓ Worktree created: {worktree_path:?} (branch: {branch_name})");

            // Open IDE if requested
            if config.open_editor {
                if let Err(e) = open_ide_in_path(&worktree_path.to_string_lossy(), "cursor") {
                    println!("⚠️  Warning: Failed to open IDE: {e}");
                    println!("   Continuing with task execution...");
                }
            }

            worktree_path.to_string_lossy().to_string()
        }
    };
    println!();

    // Create Docker manager
    let docker_manager = DockerManager::new().context("Failed to create Docker manager")?;

    // Check if claude-task-home volume exists, run setup if it doesn't
    if config.debug {
        println!("🔍 Checking if claude-task-home volume exists...");
    }
    let home_volume_exists = docker_manager.check_home_volume_exists().await?;
    if config.debug {
        println!("   Volume exists: {home_volume_exists}");
    }

    if !home_volume_exists {
        println!("🔧 claude-task-home volume not found, running setup...");
        setup_credentials_and_config(config.task_base_home_dir, config.debug).await?;
        println!();
    } else if config.debug {
        println!("✓ claude-task-home volume found");
    }

    // Create task configuration
    let mut claude_config = ClaudeTaskConfig {
        task_id: task_id.clone(),
        workspace_path: workspace_path.clone(),
        ht_mcp_port: config.ht_mcp_port,
        web_view_proxy_port: config.web_view_proxy_port,
        ..ClaudeTaskConfig::default()
    };

    if config.debug {
        println!("🔍 Docker configuration:");
        println!("   - Task ID: {}", claude_config.task_id);
        println!("   - Workspace path: {}", claude_config.workspace_path);
        println!("   - Timezone: {}", claude_config.timezone);
        if let Some(port) = claude_config.ht_mcp_port {
            println!("   - HT-MCP port: {port}");
            println!("   - NGINX proxy (container): 0.0.0.0:4618 -> 127.0.0.1:3618");
        }
        println!(
            "   - Web view proxy port (host): {}",
            claude_config.web_view_proxy_port
        );
    }

    // Create volumes (npm and node cache)
    docker_manager.create_volumes(&claude_config).await?;

    // Build image if requested, otherwise check if image exists
    if config.build {
        // Only validate Dockerfile paths when building
        if current_dir.join("claude-task/Dockerfile").exists() {
            claude_config.dockerfile_path = current_dir
                .join("claude-task/Dockerfile")
                .to_string_lossy()
                .to_string();
            claude_config.context_path = current_dir
                .join("claude-task")
                .to_string_lossy()
                .to_string();
        } else if current_dir
            .parent()
            .unwrap_or(&current_dir)
            .join("claude-task/Dockerfile")
            .exists()
        {
            let parent = current_dir.parent().unwrap_or(&current_dir);
            claude_config.dockerfile_path = parent
                .join("claude-task/Dockerfile")
                .to_string_lossy()
                .to_string();
            claude_config.context_path = parent.join("claude-task").to_string_lossy().to_string();
        } else {
            return Err(anyhow::anyhow!(
                "Dockerfile not found in ./claude-task/ or ../claude-task/\nMake sure you're running this from the correct directory."
            ));
        }
        docker_manager.build_image(&claude_config).await?;
    } else {
        // Check if the image exists, if not suggest using --build
        if docker_manager
            .check_image_exists("claude-task:dev")
            .await
            .is_err()
        {
            println!("⚠️  Image 'claude-task:dev' not found.");
            println!("   Use '--build' flag to build the image first, or build it manually:");
            println!("   docker build -t claude-task:dev ./claude-task/");
            return Err(anyhow::anyhow!(
                "Image 'claude-task:dev' not found. Use --build flag to build it."
            ));
        }
        println!("✓ Using existing image: claude-task:dev");
    }

    // Run Claude task
    docker_manager
        .run_claude_task(
            &claude_config,
            config.prompt,
            &permission_tool_arg,
            config.debug,
            validated_mcp_config,
            skip_permissions,
        )
        .await?;

    println!("   Task ID: {task_id}");
    println!("   Shared volume: claude-task-home");

    Ok(())
}

async fn list_docker_volumes() -> Result<()> {
    println!("📦 Listing Claude task Docker volumes...");

    let docker_manager = DockerManager::new().context("Failed to create Docker manager")?;

    let volumes = docker_manager.list_claude_volumes().await?;

    if volumes.is_empty() {
        println!("No Claude task volumes found.");
    } else {
        println!("Found {} Claude task volumes:", volumes.len());
        for (name, size) in volumes {
            println!("  📁 {name} ({size})");
        }
    }

    Ok(())
}

async fn clean_shared_volumes(debug: bool) -> Result<()> {
    println!("🧹 Cleaning all shared Docker volumes...");
    if debug {
        println!("🔍 Will remove all three shared volumes");
    }
    println!();

    let volume_names = vec![
        "claude-task-home",
        "claude-task-npm-cache",
        "claude-task-node-cache",
    ];

    for volume_name in &volume_names {
        let output = Command::new("docker")
            .args(["volume", "rm", volume_name])
            .output()
            .context("Failed to execute docker volume rm command")?;

        if output.status.success() {
            println!("✓ Volume '{volume_name}' removed");
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("no such volume") {
                println!("⚠️  Volume '{volume_name}' not found");
            } else {
                eprintln!("❌ Failed to remove volume '{volume_name}': {stderr}");
            }
        }
    }

    println!();
    println!("✅ Shared volume cleanup completed");
    println!("   All Claude task volumes have been removed");

    Ok(())
}

async fn clean_all_worktrees(
    branch_prefix: &str,
    skip_confirmation: bool,
    force: bool,
) -> Result<()> {
    println!("🧹 Finding all worktrees to clean up...");
    println!("Branch prefix: '{branch_prefix}'");
    println!();

    // Get list of worktrees
    let worktrees = get_matching_worktrees(branch_prefix)?;

    if worktrees.is_empty() {
        println!("No worktrees found matching branch prefix '{branch_prefix}'.");
        return Ok(());
    }

    // Check cleanliness of each worktree
    let mut worktree_status_list = Vec::new();
    let mut clean_count = 0;
    let mut unclean_count = 0;

    for (path, head, branch) in &worktrees {
        let path_buf = PathBuf::from(path);
        let status = check_worktree_status(&path_buf).ok();

        if let Some(ref s) = status {
            if s.is_clean() {
                clean_count += 1;
            } else {
                unclean_count += 1;
            }
        }

        worktree_status_list.push((path.clone(), head.clone(), branch.clone(), status));
    }

    // Display what will be cleaned
    println!("📋 Found {} worktrees:", worktree_status_list.len());
    println!("   ✅ {clean_count} clean");
    println!("   ⚠️  {unclean_count} unclean");
    println!();

    for (i, (path, _, branch, status)) in worktree_status_list.iter().enumerate() {
        let clean_branch = if branch.starts_with("refs/heads/") {
            branch.strip_prefix("refs/heads/").unwrap_or(branch)
        } else {
            branch
        };

        let (status_icon, cleanup_indicator) = if let Some(s) = status {
            if s.is_clean() {
                (
                    "✅",
                    if force || unclean_count == 0 {
                        ""
                    } else {
                        " (will clean)"
                    },
                )
            } else if force {
                ("⚠️", " (will force clean)")
            } else {
                ("⚠️", "")
            }
        } else {
            ("❓", "")
        };

        print!("  {}. Branch: {} (Path: {})", i + 1, clean_branch, path);

        if let Some(s) = status {
            if s.is_clean() {
                print!(" {status_icon} Clean{cleanup_indicator}");
            } else {
                let details = s.get_status_details();
                print!(
                    " {status_icon} Unclean: {}{cleanup_indicator}",
                    details.join(", ")
                );
            }
        } else {
            print!(" {status_icon} Status unknown");
        }

        println!();
    }

    // Determine what we're going to clean
    let (worktrees_to_clean, action_description) = if force {
        (
            worktree_status_list.len(),
            "all worktrees (including unclean ones)",
        )
    } else if unclean_count > 0 {
        (clean_count, "clean worktrees only")
    } else {
        (clean_count, "all worktrees")
    };

    if worktrees_to_clean == 0 {
        println!();
        println!("ℹ️  No clean worktrees to remove.");
        if unclean_count > 0 {
            println!("   Use --force flag to remove unclean worktrees:");
            println!("   ct worktree clean --force");
        }
        return Ok(());
    }

    // Ask for confirmation unless skipped
    if !skip_confirmation {
        println!();

        // Show info about unclean worktrees if any exist and not forcing
        if unclean_count > 0 && !force {
            println!("ℹ️  Unclean worktrees require --force flag to remove.");
        }

        print!(
            "❓ Are you sure you want to delete {worktrees_to_clean} {action_description}? [y/N]: "
        );
        use std::io::{self, Write};
        io::stdout().flush().context("Failed to flush stdout")?;

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .context("Failed to read input")?;

        let input = input.trim().to_lowercase();
        if input != "y" && input != "yes" {
            println!("❌ Cleanup cancelled.");
            return Ok(());
        }
    }

    println!("🧹 Starting cleanup...");
    println!();

    // Clean up each worktree (only clean ones unless force is used)
    let mut cleaned_count = 0;
    let mut skipped_count = 0;

    for (_, _, branch, status) in worktree_status_list.iter() {
        let clean_branch = if branch.starts_with("refs/heads/") {
            branch.strip_prefix("refs/heads/").unwrap_or(branch)
        } else {
            branch
        };

        if let Some(task_id) = clean_branch.strip_prefix(branch_prefix) {
            if !task_id.is_empty() {
                let should_clean = if let Some(s) = status {
                    force || s.is_clean()
                } else {
                    // If status unknown, only clean with force
                    force
                };

                if should_clean {
                    let status_warning = if let Some(s) = status {
                        if !s.is_clean() {
                            " (⚠️  Unclean - forced removal)"
                        } else {
                            ""
                        }
                    } else {
                        ""
                    };

                    println!(
                        "🗑️  [{}/{}] Cleaning up task '{}'{}...",
                        cleaned_count + 1,
                        worktrees_to_clean,
                        task_id,
                        status_warning
                    );

                    // Remove worktree (this will also delete the branch)
                    if let Err(e) = remove_git_worktree(task_id, branch_prefix) {
                        println!("⚠️  Failed to remove worktree for '{task_id}': {e}");
                    } else {
                        println!("✓ Worktree removed for task '{task_id}'");
                        cleaned_count += 1;
                    }

                    println!();
                } else {
                    // Skip unclean worktrees when not using force
                    skipped_count += 1;
                    if skipped_count == 1 {
                        println!("⏭️  Skipping unclean worktrees (use --force to clean them):");
                    }
                    println!("   - {task_id}");
                }
            }
        }
    }

    if skipped_count > 0 {
        println!();
    }

    println!("✅ Worktree cleanup completed!");
    println!("   Cleaned: {cleaned_count} worktrees");
    if skipped_count > 0 {
        println!("   Skipped: {skipped_count} unclean worktrees");
    }

    Ok(())
}

async fn clean_all_worktrees_and_volumes(
    branch_prefix: &str,
    skip_confirmation: bool,
    force: bool,
) -> Result<()> {
    println!("🧹 Cleaning up both worktrees and volumes...");
    println!();

    // Clean worktrees first
    clean_all_worktrees(branch_prefix, skip_confirmation, force).await?;

    println!();
    println!("🐋 Now cleaning Docker volumes...");
    println!();

    // Then clean volumes
    clean_shared_volumes(false).await?;

    println!();
    println!("✅ Complete cleanup finished!");
    println!("   Both worktrees and volumes have been cleaned");

    Ok(())
}

fn get_matching_worktrees(branch_prefix: &str) -> Result<Vec<(String, String, String)>> {
    let current_dir = std::env::current_dir().context("Could not get current directory")?;
    let repo_root = find_git_repo_root(&current_dir)?;

    let output = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(&repo_root)
        .output()
        .context("Failed to execute git worktree list command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!(
            "Git worktree list command failed: {}",
            stderr
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();

    if lines.is_empty() {
        return Ok(Vec::new());
    }

    let mut current_worktree: Option<(String, String, String)> = None; // (path, head, branch)
    let mut matching_worktrees = Vec::new();

    for line in lines {
        if line.starts_with("worktree ") {
            // If we have a previous worktree, check if it matches and store it
            if let Some((path, head, branch)) = current_worktree.take() {
                if should_clean_worktree(&branch, branch_prefix, &path, &repo_root) {
                    matching_worktrees.push((path, head, branch));
                }
            }

            // Start new worktree
            let path = line.strip_prefix("worktree ").unwrap_or(line);
            current_worktree = Some((path.to_string(), String::new(), String::new()));
        } else if line.starts_with("HEAD ") {
            if let Some((_, ref mut head, _)) = current_worktree.as_mut() {
                let new_head = line.strip_prefix("HEAD ").unwrap_or(line);
                *head = new_head.to_string();
            }
        } else if line.starts_with("branch ") {
            if let Some((_, _, ref mut branch)) = current_worktree.as_mut() {
                let new_branch = line.strip_prefix("branch ").unwrap_or(line);
                *branch = new_branch.to_string();
            }
        } else if line == "bare" {
            if let Some((_, _, ref mut branch)) = current_worktree.as_mut() {
                *branch = "(bare)".to_string();
            }
        } else if line == "detached" {
            if let Some((_, _, ref mut branch)) = current_worktree.as_mut() {
                *branch = "(detached)".to_string();
            }
        }
    }

    // Handle the last worktree if it exists
    if let Some((path, head, branch)) = current_worktree {
        if should_clean_worktree(&branch, branch_prefix, &path, &repo_root) {
            matching_worktrees.push((path, head, branch));
        }
    }

    Ok(matching_worktrees)
}

fn should_clean_worktree(
    branch: &str,
    branch_prefix: &str,
    path: &str,
    repo_root: &std::path::Path,
) -> bool {
    // Clean up branch name by removing refs/heads/ prefix for comparison
    let clean_branch = if branch.starts_with("refs/heads/") {
        branch.strip_prefix("refs/heads/").unwrap_or(branch)
    } else {
        branch
    };

    // Exclude the main repository directory
    let worktree_path = std::path::Path::new(path);
    if worktree_path == repo_root {
        return false;
    }

    // Only include branches that start with the prefix (exclude main/master and special states)
    clean_branch.starts_with(branch_prefix)
}

#[derive(Debug)]
pub struct WorktreeStatus {
    pub has_uncommitted_changes: bool,
    pub has_unpushed_commits: bool,
    pub has_no_remote: bool,
    pub current_branch: String,
    pub remote_branch: Option<String>,
    pub ahead_count: usize,
    pub behind_count: usize,
    pub changed_files: Vec<String>,
    pub untracked_files: Vec<String>,
    pub unpushed_commits: Vec<(String, String)>, // (commit_id, message)
    pub is_likely_merged: bool,
    pub merge_info: Option<String>, // e.g., "squash-merged", "merged", "PR #123"
}

impl WorktreeStatus {
    pub fn is_clean(&self) -> bool {
        !self.has_uncommitted_changes
            && (!self.has_unpushed_commits || self.is_likely_merged)
            && (!self.has_no_remote || self.is_likely_merged)
    }

    pub fn get_status_icon(&self) -> &'static str {
        if self.is_clean() {
            "✅"
        } else {
            "⚠️"
        }
    }

    pub fn get_status_details(&self) -> Vec<String> {
        let mut details = Vec::new();

        if self.has_uncommitted_changes {
            details.push("uncommitted changes".to_string());
        }

        if self.has_unpushed_commits && self.ahead_count > 0 {
            details.push(format!("{} unpushed commits", self.ahead_count));
        }

        if self.behind_count > 0 {
            details.push(format!("{} commits behind remote", self.behind_count));
        }

        if self.has_no_remote {
            details.push("no remote tracking branch".to_string());
        }

        details
    }
}

fn check_if_branch_merged(branch: &str, worktree_path: &Path) -> (bool, Option<String>) {
    // Try to detect if this branch has been merged into main/master

    // First, find the main branch (main or master)
    let main_branches = ["main", "master"];
    let mut main_branch = None;

    for mb in &main_branches {
        let output = Command::new("git")
            .args(["rev-parse", "--verify", mb])
            .current_dir(worktree_path)
            .output();

        if let Ok(output) = output {
            if output.status.success() {
                main_branch = Some(*mb);
                break;
            }
        }
    }

    let main_branch = match main_branch {
        Some(mb) => mb,
        None => return (false, None), // Can't detect without a main branch
    };

    // Method 1: Check if branch is in --merged list (regular merge)
    if let Ok(output) = Command::new("git")
        .args(["branch", "--merged", main_branch])
        .current_dir(worktree_path)
        .output()
    {
        if output.status.success() {
            let output_str = String::from_utf8_lossy(&output.stdout);
            for line in output_str.lines() {
                let line = line.trim().trim_start_matches('*').trim();
                if line == branch {
                    return (true, Some("merged".to_string()));
                }
            }
        }
    }

    // Method 2: Check if all changes are already in main (squash merge detection)
    // This compares the diff between the merge-base and branch tip
    if let Ok(merge_base_output) = Command::new("git")
        .args(["merge-base", main_branch, branch])
        .current_dir(worktree_path)
        .output()
    {
        if merge_base_output.status.success() {
            let merge_base = String::from_utf8_lossy(&merge_base_output.stdout)
                .trim()
                .to_string();

            // Check if there are any changes between merge-base..branch that aren't in main
            if let Ok(diff_output) = Command::new("git")
                .args(["diff", "--exit-code", &format!("{merge_base}..{branch}")])
                .current_dir(worktree_path)
                .output()
            {
                if diff_output.status.success() {
                    // No diff means no changes
                    return (true, Some("no changes".to_string()));
                }

                // There are changes, check if they're already in main using git log --grep
                // First, get the commit messages from the branch
                if let Ok(log_output) = Command::new("git")
                    .args(["log", "--oneline", &format!("{merge_base}..{branch}")])
                    .current_dir(worktree_path)
                    .output()
                {
                    if log_output.status.success() {
                        let log_str = String::from_utf8_lossy(&log_output.stdout);
                        let commit_count = log_str.lines().count();

                        if commit_count > 0 {
                            // Check if main has any commits that might be squash merges of this branch
                            // Look for commits that mention the branch name or PR
                            if let Ok(main_log) = Command::new("git")
                                .args([
                                    "log",
                                    "--oneline",
                                    "--grep",
                                    &format!("{branch}\\|#[0-9]\\+"),
                                    &format!("{merge_base}..{main_branch}"),
                                ])
                                .current_dir(worktree_path)
                                .output()
                            {
                                if main_log.status.success() && !main_log.stdout.is_empty() {
                                    return (true, Some("likely squash-merged".to_string()));
                                }
                            }

                            // Alternative: Check if the file changes are already in main
                            // Get list of files changed in the branch
                            if let Ok(files_output) = Command::new("git")
                                .args(["diff", "--name-only", &format!("{merge_base}..{branch}")])
                                .current_dir(worktree_path)
                                .output()
                            {
                                if files_output.status.success() {
                                    let files = String::from_utf8_lossy(&files_output.stdout);
                                    let file_count = files.lines().count();

                                    if file_count > 0 {
                                        // For each file, check if its content in branch matches main
                                        let mut all_changes_in_main = true;

                                        for file in files.lines() {
                                            if !file.is_empty() {
                                                // Compare file content between branch and main
                                                if let Ok(diff) = Command::new("git")
                                                    .args([
                                                        "diff",
                                                        "--no-index",
                                                        "--quiet",
                                                        &format!("{branch}:{file}"),
                                                        &format!("{main_branch}:{file}"),
                                                    ])
                                                    .current_dir(worktree_path)
                                                    .output()
                                                {
                                                    if !diff.status.success() {
                                                        // Files differ
                                                        all_changes_in_main = false;
                                                        break;
                                                    }
                                                }
                                            }
                                        }

                                        if all_changes_in_main && commit_count > 1 {
                                            // Multiple commits but all changes are in main = likely squash merge
                                            return (
                                                true,
                                                Some("likely squash-merged".to_string()),
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Method 3: Try GitHub CLI if available to check PR status
    if let Ok(output) = Command::new("gh")
        .args([
            "pr",
            "list",
            "--state",
            "merged",
            "--head",
            branch,
            "--json",
            "number,title",
        ])
        .current_dir(worktree_path)
        .output()
    {
        if output.status.success() && !output.stdout.is_empty() {
            let json_str = String::from_utf8_lossy(&output.stdout);
            if json_str.contains("number") {
                // Simple check - if there's a merged PR for this branch
                return (true, Some("PR merged".to_string()));
            }
        }
    }

    (false, None)
}

pub fn check_worktree_status(worktree_path: &Path) -> Result<WorktreeStatus> {
    // Check for uncommitted changes and get file lists
    let status_output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(worktree_path)
        .output()
        .context("Failed to execute git status command")?;

    if !status_output.status.success() {
        let stderr = String::from_utf8_lossy(&status_output.stderr);
        return Err(anyhow::anyhow!("Git status command failed: {}", stderr));
    }

    let status_str = String::from_utf8_lossy(&status_output.stdout);
    let mut changed_files = Vec::new();
    let mut untracked_files = Vec::new();

    for line in status_str.lines() {
        if line.len() >= 3 {
            let status_code = &line[0..2];
            let file_path = line[3..].trim();

            if status_code.contains('?') {
                untracked_files.push(file_path.to_string());
            } else {
                changed_files.push(format!("{} {}", status_code.trim(), file_path));
            }
        }
    }

    let has_uncommitted_changes = !changed_files.is_empty() || !untracked_files.is_empty();

    // Get current branch
    let branch_output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(worktree_path)
        .output()
        .context("Failed to get current branch")?;

    let current_branch = if branch_output.status.success() {
        String::from_utf8_lossy(&branch_output.stdout)
            .trim()
            .to_string()
    } else {
        "unknown".to_string()
    };

    // Check if branch has a remote tracking branch
    let remote_output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"])
        .current_dir(worktree_path)
        .output()
        .context("Failed to check remote tracking branch")?;

    let (has_no_remote, remote_branch) = if remote_output.status.success() {
        let remote = String::from_utf8_lossy(&remote_output.stdout)
            .trim()
            .to_string();
        (false, Some(remote))
    } else {
        (true, None)
    };

    // Check ahead/behind status and get unpushed commits
    let (ahead_count, behind_count, has_unpushed_commits, unpushed_commits) = if !has_no_remote {
        let rev_list_output = Command::new("git")
            .args(["rev-list", "--left-right", "--count", "HEAD...@{u}"])
            .current_dir(worktree_path)
            .output()
            .context("Failed to check ahead/behind status")?;

        if rev_list_output.status.success() {
            let output = String::from_utf8_lossy(&rev_list_output.stdout);
            let parts: Vec<&str> = output.trim().split('\t').collect();

            let ahead = parts
                .first()
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(0);
            let behind = parts
                .get(1)
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(0);

            // Get unpushed commits if there are any
            let mut commits = Vec::new();
            if ahead > 0 {
                let log_output = Command::new("git")
                    .args(["log", "--oneline", "@{u}..HEAD"])
                    .current_dir(worktree_path)
                    .output()
                    .context("Failed to get unpushed commits")?;

                if log_output.status.success() {
                    let log_str = String::from_utf8_lossy(&log_output.stdout);
                    for line in log_str.lines() {
                        if let Some(space_pos) = line.find(' ') {
                            let commit_id = line[..space_pos].to_string();
                            let message = line[space_pos + 1..].to_string();
                            commits.push((commit_id, message));
                        }
                    }
                }
            }

            (ahead, behind, ahead > 0, commits)
        } else {
            (0, 0, false, Vec::new())
        }
    } else {
        // If no remote, check if we have any commits
        let log_output = Command::new("git")
            .args(["log", "--oneline", "-10"]) // Get last 10 commits if no remote
            .current_dir(worktree_path)
            .output()
            .context("Failed to check commits")?;

        let mut commits = Vec::new();
        if log_output.status.success() && !log_output.stdout.is_empty() {
            let log_str = String::from_utf8_lossy(&log_output.stdout);
            for line in log_str.lines() {
                if let Some(space_pos) = line.find(' ') {
                    let commit_id = line[..space_pos].to_string();
                    let message = line[space_pos + 1..].to_string();
                    commits.push((commit_id, message));
                }
            }
        }

        let has_commits = !commits.is_empty();
        (commits.len(), 0, has_commits, commits)
    };

    // Check if branch has been merged (only if it has unpushed commits or no remote)
    let (is_likely_merged, merge_info) = if has_unpushed_commits || has_no_remote {
        check_if_branch_merged(&current_branch, worktree_path)
    } else {
        (false, None)
    };

    Ok(WorktreeStatus {
        has_uncommitted_changes,
        has_unpushed_commits,
        has_no_remote,
        current_branch,
        remote_branch,
        ahead_count,
        behind_count,
        changed_files,
        untracked_files,
        unpushed_commits,
        is_likely_merged,
        merge_info,
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Setup) => {
            setup_credentials_and_config(&cli.task_base_home_dir, cli.debug).await?;
        }
        Some(Commands::Worktree { command }) => match command {
            WorktreeCommands::Create { task_id } => {
                let (_worktree_path, _branch_name) =
                    create_git_worktree(&task_id, &cli.branch_prefix, &cli.worktree_base_dir)?;
            }
            WorktreeCommands::List => {
                list_git_worktrees(&cli.branch_prefix)?;
            }
            WorktreeCommands::Remove { task_id } => {
                remove_git_worktree(&task_id, &cli.branch_prefix)?;
            }
            WorktreeCommands::Open => {
                select_worktree_interactively(&cli.branch_prefix)?;
            }
            WorktreeCommands::Clean { yes, force } => {
                clean_all_worktrees(&cli.branch_prefix, yes, force).await?;
            }
        },
        Some(Commands::Docker { command }) => match command {
            DockerCommands::Init {
                refresh_credentials,
            } => {
                init_shared_volumes(refresh_credentials, &cli.task_base_home_dir, cli.debug)
                    .await?;
            }
            DockerCommands::List => {
                list_docker_volumes().await?;
            }
            DockerCommands::Clean => {
                clean_shared_volumes(cli.debug).await?;
            }
        },
        Some(Commands::Run {
            prompt,
            task_id,
            build,
            workspace_dir,
            approval_tool_permission,
            mcp_config,
            yes,
            open_editor,
            ht_mcp_port,
            web_view_proxy_port,
        }) => {
            let debug_mode = cli.debug; // Use global debug flag
            let config = TaskRunConfig {
                prompt: &prompt,
                task_id,
                build,
                workspace_dir,
                approval_tool_permission,
                debug: debug_mode,
                mcp_config,
                skip_confirmation: yes,
                worktree_base_dir: &cli.worktree_base_dir,
                task_base_home_dir: &cli.task_base_home_dir,
                open_editor,
                ht_mcp_port,
                web_view_proxy_port,
            };
            run_claude_task(config).await?;
        }
        Some(Commands::Clean { yes, force }) => {
            clean_all_worktrees_and_volumes(&cli.branch_prefix, yes, force).await?;
        }
        Some(Commands::Mcp) => {
            mcp::run_mcp_server().await?;
        }
        Some(Commands::Version) => {
            println!("claude-task {}", env!("CARGO_PKG_VERSION"));
        }
        None => {
            // Default behavior: show help
            let mut cmd = Cli::command();
            cmd.print_help().context("Failed to print help")?;
        }
    }

    Ok(())
}
