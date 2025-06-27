use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand};
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

mod credentials;
mod docker;

use credentials::setup_credentials_and_config;
use docker::{ClaudeTaskConfig, DockerManager};

#[derive(Subcommand)]
enum WorktreeCommands {
    /// Create a new git worktree
    Create {
        /// Task ID for the worktree
        task_id: String,
    },
    /// List current git worktrees
    List,
    /// Remove and clean up a worktree
    Remove {
        /// Task ID to remove (will be prefixed with branch_prefix)
        task_id: String,
    },
}

#[derive(Subcommand)]
enum VolumeCommands {
    /// Initialize shared docker volumes for Claude tasks
    Init {
        /// Refresh credentials by running setup first
        #[arg(long)]
        refresh_credentials: bool,
    },
    /// List Docker volumes for Claude tasks
    List,
    /// Clean up all shared Docker volumes
    Clean,
}

#[derive(Parser)]
#[command(name = "claude-setup")]
#[command(about = "Claude setup and git worktree management")]
struct Cli {
    /// Base directory for worktrees
    #[arg(long, global = true, default_value = "~/.claude-task/worktrees")]
    worktree_base_dir: String,

    /// Branch prefix for worktrees
    #[arg(long, global = true, default_value = "claude-task/")]
    branch_prefix: String,

    /// Base directory for task home directory and setup files
    #[arg(long, global = true, default_value = "~/.claude-task/home")]
    task_base_home_dir: String,

    /// Enable debug mode
    #[arg(long, global = true)]
    debug: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Setup claude-task with your current environment
    Setup,
    /// Git worktree management commands
    Worktree {
        #[command(subcommand)]
        command: WorktreeCommands,
    },
    /// Docker volume management commands
    Volume {
        #[command(subcommand)]
        command: VolumeCommands,
    },
    /// Run a Claude task in a local docker container
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
        #[arg(long, value_name = "PERMISSION_STATEMENT")]
        approval_tool_permission: Option<String>,
        /// Enable debug mode for Claude command
        #[arg(long)]
        debug: bool,
        /// Optional MCP config file path that will be mounted to the container and passed to Claude
        #[arg(long, value_name = "MCP_CONFIG_FILEPATH")]
        mcp_config: Option<String>,
        /// Skip confirmation prompts (automatically answer yes)
        #[arg(long, short)]
        yes: bool,
    },
    /// Clean up all claude-task git worktrees and docker volumes
    Clean {
        /// Skip confirmation prompt
        #[arg(long)]
        yes: bool,
    },
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
        .with_context(|| format!("Failed to create worktree directory: {:?}", worktree_path))?;
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
    let branch_name = format!("{}{}", branch_prefix, sanitized_name);

    let worktree_base_dir = get_worktree_directory(worktree_base_dir)?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let worktree_path = worktree_base_dir.join(format!("{}_{:x}", sanitized_name, timestamp));

    println!("Creating git worktree...");
    println!("Repository root: {:?}", repo_root);
    println!("Branch name: {}", branch_name);
    println!("Worktree path: {:?}", worktree_path);

    // Create the worktree
    let output = Command::new("git")
        .args(&["worktree", "add", "-b", &branch_name])
        .arg(&worktree_path)
        .current_dir(&repo_root)
        .output()
        .context("Failed to execute git worktree command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("Git worktree command failed: {}", stderr));
    }

    println!("✓ Git worktree created successfully");
    println!("  Branch: {}", branch_name);
    println!("  Path: {:?}", worktree_path);

    Ok((worktree_path, branch_name))
}

fn list_git_worktrees(branch_prefix: &str) -> Result<()> {
    let current_dir = std::env::current_dir().context("Could not get current directory")?;
    let repo_root = find_git_repo_root(&current_dir)?;

    println!(
        "Listing git worktrees with branch prefix '{}'...",
        branch_prefix
    );
    println!("Repository root: {:?}", repo_root);
    println!();

    let output = Command::new("git")
        .args(&["worktree", "list", "--porcelain"])
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
        println!(
            "No worktrees found matching branch prefix '{}'.",
            branch_prefix
        );
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

    println!("{} {}{}", icon, dir_name, type_label);
    println!("   Path: {}", path);
    println!("   Branch: {}", clean_branch);
    println!(
        "   HEAD: {}",
        if head.len() > 7 { &head[..7] } else { head }
    );
    println!();
}

fn remove_git_worktree(task_id: &str, branch_prefix: &str) -> Result<()> {
    let current_dir = std::env::current_dir().context("Could not get current directory")?;
    let repo_root = find_git_repo_root(&current_dir)?;

    let sanitized_id = sanitize_branch_name(task_id);
    let branch_name = format!("{}{}", branch_prefix, sanitized_id);

    println!("Removing git worktree for task '{}'...", task_id);
    println!("Repository root: {:?}", repo_root);
    println!("Target branch: {}", branch_name);
    println!();

    // First, get list of worktrees to find the one with matching branch
    let output = Command::new("git")
        .args(&["worktree", "list", "--porcelain"])
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
            println!("❌ No worktree found for branch '{}'", branch_name);
            return Ok(());
        }
    };

    println!("Found worktree: {}", worktree_path);

    // Remove the worktree
    println!("Removing worktree...");
    let output = Command::new("git")
        .args(&["worktree", "remove", &worktree_path, "--force"])
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

    println!("✓ Worktree removed: {}", worktree_path);

    // Delete the branch
    println!("Deleting branch '{}'...", branch_name);
    let output = Command::new("git")
        .args(&["branch", "-D", &branch_name])
        .current_dir(&repo_root)
        .output()
        .context("Failed to execute git branch delete command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!(
            "⚠️  Warning: Failed to delete branch '{}': {}",
            branch_name, stderr
        );
        println!(
            "   You may need to delete it manually with: git branch -D {}",
            branch_name
        );
    } else {
        println!("✓ Branch deleted: {}", branch_name);
    }

    println!();
    println!("✅ Cleanup complete for task '{}'", task_id);

    Ok(())
}

async fn init_shared_volumes(
    refresh_credentials: bool,
    task_base_home_dir: &str,
    debug: bool,
) -> Result<()> {
    println!("Initializing shared Docker volumes for Claude tasks...");
    if debug {
        println!("🔍 Refresh credentials: {}", refresh_credentials);
        println!("🔍 Task base home dir: {}", task_base_home_dir);
    }
    println!();

    // Create Docker manager
    let docker_manager = DockerManager::new().context("Failed to create Docker manager")?;

    // Create cache volumes (npm and node)
    println!("Creating cache volumes...");
    let dummy_config = ClaudeTaskConfig::default();
    docker_manager.create_volumes(&dummy_config).await?;

    // Run setup if requested or ensure claude-task-home exists
    if refresh_credentials {
        println!("Refreshing credentials...");
        setup_credentials_and_config(task_base_home_dir, debug).await?;
    } else {
        // Check if claude-task-home exists, create it if not
        if !docker_manager.check_home_volume_exists().await? {
            println!("claude-task-home volume not found, running setup...");
            setup_credentials_and_config(task_base_home_dir, debug).await?;
        } else {
            println!("✓ claude-task-home volume already exists");
        }
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

async fn run_claude_task(
    prompt: &str,
    task_id: Option<String>,
    build: bool,
    workspace_dir: Option<Option<String>>,
    approval_tool_permission: Option<String>,
    debug: bool,
    mcp_config: Option<String>,
    skip_confirmation: bool,
    worktree_base_dir: &str,
    task_base_home_dir: &str,
) -> Result<()> {
    if debug {
        println!("🔍 Debug mode enabled");
        println!("📝 Task parameters:");
        println!("   - Prompt: {}", prompt);
        println!("   - Task ID: {:?}", task_id);
        println!("   - Build: {}", build);
        println!("   - Workspace dir: {:?}", workspace_dir);
        println!(
            "   - Approval tool permission: {:?}",
            approval_tool_permission
        );
        println!("   - MCP config: {:?}", mcp_config);
        println!("   - Worktree base dir: {}", worktree_base_dir);
        println!("   - Task base home dir: {}", task_base_home_dir);
        println!();
    }

    let current_dir = std::env::current_dir().context("Could not get current directory")?;

    // Validate MCP config file if provided
    let validated_mcp_config = if let Some(mcp_path) = mcp_config {
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

        if debug {
            println!("🔍 MCP config file found: {}", mcp_config_path.display());
        }

        Some(mcp_config_path.to_string_lossy().to_string())
    } else {
        None
    };

    // Handle approval tool permission configuration FIRST, before any setup
    let (permission_tool_arg, skip_permissions) = match approval_tool_permission {
        Some(tool) => (tool, false),
        None => {
            // Show warning and request confirmation
            println!("⚠️  WARNING: No approval tool permission specified!");
            println!("   This will run Claude with --dangerously-skip-permissions");
            println!("   Claude will have unrestricted access to execute commands without user approval.");
            println!("   This is DANGEROUS and should only be used in trusted environments.");
            println!();

            if !skip_confirmation {
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

    // Generate or use provided task ID
    let task_id = match task_id {
        Some(id) => id,
        None => generate_short_id(),
    };

    println!("Running Claude task with ID: {}", task_id);
    println!("Prompt: {}", prompt);
    println!();

    // Determine workspace directory
    let workspace_path = match workspace_dir {
        Some(Some(custom_dir)) => {
            // Use custom directory provided
            let custom_path = PathBuf::from(&custom_dir);
            if !custom_path.exists() {
                return Err(anyhow::anyhow!(
                    "Custom workspace directory does not exist: {}",
                    custom_dir
                ));
            }
            println!("📁 Using custom workspace directory: {}", custom_dir);
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
                create_git_worktree(&task_id, "claude-task/", worktree_base_dir)?;
            println!(
                "✓ Worktree created: {:?} (branch: {})",
                worktree_path, branch_name
            );
            worktree_path.to_string_lossy().to_string()
        }
    };
    println!();

    // Create Docker manager
    let docker_manager = DockerManager::new().context("Failed to create Docker manager")?;

    // Check if claude-task-home volume exists, run setup if it doesn't
    if debug {
        println!("🔍 Checking if claude-task-home volume exists...");
    }
    let home_volume_exists = docker_manager.check_home_volume_exists().await?;
    if debug {
        println!("   Volume exists: {}", home_volume_exists);
    }

    if !home_volume_exists {
        println!("🔧 claude-task-home volume not found, running setup...");
        setup_credentials_and_config(task_base_home_dir, debug).await?;
        println!();
    } else if debug {
        println!("✓ claude-task-home volume found");
    }

    // Create task configuration
    let mut config = ClaudeTaskConfig::default();
    config.task_id = task_id.clone();
    config.workspace_path = workspace_path.clone();

    if debug {
        println!("🔍 Docker configuration:");
        println!("   - Task ID: {}", config.task_id);
        println!("   - Workspace path: {}", config.workspace_path);
        println!("   - Timezone: {}", config.timezone);
    }

    // Create volumes (npm and node cache)
    docker_manager.create_volumes(&config).await?;

    // Build image if requested, otherwise check if image exists
    if build {
        // Only validate Dockerfile paths when building
        if current_dir.join("claude-task/Dockerfile").exists() {
            config.dockerfile_path = current_dir
                .join("claude-task/Dockerfile")
                .to_string_lossy()
                .to_string();
            config.context_path = current_dir
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
            config.dockerfile_path = parent
                .join("claude-task/Dockerfile")
                .to_string_lossy()
                .to_string();
            config.context_path = parent.join("claude-task").to_string_lossy().to_string();
        } else {
            return Err(anyhow::anyhow!(
                "Dockerfile not found in ./claude-task/ or ../claude-task/\nMake sure you're running this from the correct directory."
            ));
        }
        docker_manager.build_image(&config).await?;
    } else {
        // Check if the image exists, if not suggest using --build
        if let Err(_) = docker_manager.check_image_exists("claude-task:dev").await {
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
            &config,
            prompt,
            &permission_tool_arg,
            debug,
            validated_mcp_config,
            skip_permissions,
        )
        .await?;

    println!("   Task ID: {}", task_id);
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
            println!("  📁 {} ({})", name, size);
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
            .args(&["volume", "rm", volume_name])
            .output()
            .context("Failed to execute docker volume rm command")?;

        if output.status.success() {
            println!("✓ Volume '{}' removed", volume_name);
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("no such volume") {
                println!("⚠️  Volume '{}' not found", volume_name);
            } else {
                eprintln!("❌ Failed to remove volume '{}': {}", volume_name, stderr);
            }
        }
    }

    println!();
    println!("✅ Shared volume cleanup completed");
    println!("   All Claude task volumes have been removed");

    Ok(())
}

async fn clean_all_worktrees_and_volumes(
    branch_prefix: &str,
    skip_confirmation: bool,
) -> Result<()> {
    println!("🧹 Finding all worktrees and volumes to clean up...");
    println!("Branch prefix: '{}'", branch_prefix);
    println!();

    // Get list of worktrees
    let worktrees = get_matching_worktrees(branch_prefix)?;

    if worktrees.is_empty() {
        println!(
            "No worktrees found matching branch prefix '{}'.",
            branch_prefix
        );
        return Ok(());
    }

    // Extract task IDs from branch names
    let mut task_ids = Vec::new();
    for (_, _, branch) in &worktrees {
        let clean_branch = if branch.starts_with("refs/heads/") {
            branch.strip_prefix("refs/heads/").unwrap_or(branch)
        } else {
            branch
        };

        if let Some(task_id) = clean_branch.strip_prefix(branch_prefix) {
            if !task_id.is_empty() {
                task_ids.push(task_id.to_string());
            }
        }
    }

    // Display what will be cleaned
    println!("📋 Found {} worktrees to clean up:", worktrees.len());
    for (i, (path, _, branch)) in worktrees.iter().enumerate() {
        let clean_branch = if branch.starts_with("refs/heads/") {
            branch.strip_prefix("refs/heads/").unwrap_or(branch)
        } else {
            branch
        };
        println!("  {}. Branch: {} (Path: {})", i + 1, clean_branch, path);
    }

    if !task_ids.is_empty() {
        println!();
        println!("📦 Legacy Docker volumes that will be cleaned (if they exist):");
        for task_id in &task_ids {
            println!(
                "  - claude-task-{}-home-dir (legacy)",
                sanitize_branch_name(task_id)
            );
        }
        println!("Note: Current system uses shared volume 'claude-task-home'");
    }

    println!();

    // Ask for confirmation unless skipped
    if !skip_confirmation {
        print!("❓ Are you sure you want to delete all these worktrees and volumes? [y/N]: ");
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

    // Create Docker manager for volume cleanup
    let docker_manager = DockerManager::new().context("Failed to create Docker manager")?;

    // Clean up each worktree and its volumes
    for (i, (_, _, branch)) in worktrees.iter().enumerate() {
        let clean_branch = if branch.starts_with("refs/heads/") {
            branch.strip_prefix("refs/heads/").unwrap_or(branch)
        } else {
            branch
        };

        if let Some(task_id) = clean_branch.strip_prefix(branch_prefix) {
            if !task_id.is_empty() {
                println!(
                    "🗑️  [{}/{}] Cleaning up task '{}'...",
                    i + 1,
                    worktrees.len(),
                    task_id
                );

                // Remove worktree (this will also delete the branch)
                if let Err(e) = remove_git_worktree(task_id, branch_prefix) {
                    println!("⚠️  Failed to remove worktree for '{}': {}", task_id, e);
                } else {
                    println!("✓ Worktree removed for task '{}'", task_id);
                }

                // Remove associated Docker volumes
                if let Err(e) = docker_manager.remove_task_volumes(task_id).await {
                    println!("⚠️  Failed to remove volumes for '{}': {}", task_id, e);
                } else {
                    println!("✓ Volumes removed for task '{}'", task_id);
                }

                println!();
            }
        }
    }

    println!("✅ Cleanup completed!");
    println!("   Processed {} worktrees", worktrees.len());
    println!("   Cleaned {} task volumes", task_ids.len());

    Ok(())
}

fn get_matching_worktrees(branch_prefix: &str) -> Result<Vec<(String, String, String)>> {
    let current_dir = std::env::current_dir().context("Could not get current directory")?;
    let repo_root = find_git_repo_root(&current_dir)?;

    let output = Command::new("git")
        .args(&["worktree", "list", "--porcelain"])
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
        },
        Some(Commands::Volume { command }) => match command {
            VolumeCommands::Init {
                refresh_credentials,
            } => {
                init_shared_volumes(refresh_credentials, &cli.task_base_home_dir, cli.debug)
                    .await?;
            }
            VolumeCommands::List => {
                list_docker_volumes().await?;
            }
            VolumeCommands::Clean => {
                clean_shared_volumes(cli.debug).await?;
            }
        },
        Some(Commands::Run {
            prompt,
            task_id,
            build,
            workspace_dir,
            approval_tool_permission,
            debug,
            mcp_config,
            yes,
        }) => {
            let debug_mode = cli.debug || debug; // Use global debug or local debug flag
            run_claude_task(
                &prompt,
                task_id,
                build,
                workspace_dir,
                approval_tool_permission,
                debug_mode,
                mcp_config,
                yes,
                &cli.worktree_base_dir,
                &cli.task_base_home_dir,
            )
            .await?;
        }
        Some(Commands::Clean { yes }) => {
            clean_all_worktrees_and_volumes(&cli.branch_prefix, yes).await?;
        }
        None => {
            // Default behavior: show help
            let mut cmd = Cli::command();
            cmd.print_help().context("Failed to print help")?;
        }
    }

    Ok(())
}
