use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand};
use std::path::{Path, PathBuf};

// Include the generated MCP help text
include!(concat!(env!("OUT_DIR"), "/mcp_help.rs"));

mod assets;
mod config;
mod credential_sync;
mod credentials;
mod docker;
mod handle_config;
mod mcp;
pub mod permission;

use claude_task::kube;
use claude_task::worktree;
use config::ExecutionEnvironment;
use permission::ApprovalToolPermission;
use std::process::Command;

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
    branch_prefix: &'a str,
    open_editor: bool,
    ht_mcp_port: Option<u16>,
    web_view_proxy_port: Option<u16>,
    require_ht_mcp: bool,
    docker_config: &'a config::DockerConfig,
    claude_user_config: &'a config::ClaudeUserConfig,
    worktree_config: &'a config::WorktreeConfig,
    async_mode: bool,
    task_runner: &'a config::ExecutionEnvironment,
    kube_config: &'a Option<config::KubeConfig>,
    git_secret_name: Option<String>,
    git_secret_key: Option<String>,
    claude_credentials: &'a Option<config::ClaudeCredentials>,
}

use config::Config;
use credentials::{setup_credentials_and_config, setup_credentials_and_config_with_cache};
use docker::{ClaudeTaskConfig, DockerManager};
use handle_config::handle_config_command;

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

#[derive(Subcommand, Clone)]
enum ConfigCommands {
    /// Create default config file
    #[command(visible_alias = "i")]
    Init {
        /// Force overwrite if config already exists
        #[arg(long, short)]
        force: bool,
    },
    /// Open config file in editor
    #[command(visible_alias = "e")]
    Edit,
    /// Display current configuration
    #[command(visible_alias = "s")]
    Show {
        /// Show config in JSON format (default: pretty print)
        #[arg(long)]
        json: bool,
    },
    /// Check config file validity
    #[command(visible_alias = "v")]
    Validate,
    /// Set the task runner (options: docker or kubernetes)
    #[command(visible_alias = "r")]
    Runner {
        /// Task runner to use
        #[arg(value_enum)]
        runner: Option<ExecutionEnvironment>,
    },
    /// Set Claude OAuth token for authentication
    #[command(visible_alias = "t")]
    Token,
}

#[derive(Subcommand)]
enum SetupCommands {
    /// Setup Docker environment (volumes and credentials)
    #[command(visible_alias = "d")]
    Docker,
    /// Setup Kubernetes environment (secrets and credentials)
    #[command(visible_alias = "k")]
    Kubernetes,
}

#[derive(Parser)]
#[command(name = "claude-task")]
#[command(about = "Claude Task Management CLI")]
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
    #[arg(short = 'd', long, global = true)]
    debug: bool,

    /// Require ht-mcp to be available for tasks (overrides config setting)
    #[arg(long, global = true)]
    require_ht_mcp: bool,

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
    Setup {
        #[command(subcommand)]
        command: SetupCommands,
    },
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
    /// Run a Claude task in a local docker container or Kubernetes
    #[command(visible_alias = "r")]
    Run {
        /// The prompt to pass to Claude
        prompt: String,
        /// Optional task ID (generates short ID if not provided)
        #[arg(short = 't', long)]
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
        /// Port to expose for web view proxy to see terminal commands the task runs
        #[arg(long)]
        web_view_proxy_port: Option<u16>,
        /// Run task in background mode (returns immediately with container ID)
        #[arg(short = 'b', long = "background")]
        async_mode: bool,
        /// Execution environment (docker or kubernetes). Overrides config setting
        #[arg(long, value_enum)]
        execution_env: Option<ExecutionEnvironment>,
        /// Kubernetes namespace to use (overrides config)
        #[arg(long)]
        kube_namespace: Option<String>,
        /// Kubernetes context to use (overrides config)
        #[arg(long)]
        kube_context: Option<String>,
        /// Name of existing Kubernetes secret containing git credentials (default: git-credentials)
        #[arg(long, value_name = "SECRET_NAME")]
        git_secret_name: Option<String>,
        /// Key within the secret containing the token (default: token)
        #[arg(long, value_name = "KEY")]
        git_secret_key: Option<String>,
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
    /// Configuration management commands
    #[command(visible_alias = "cf")]
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },
    /// Launch MCP server on stdio
    #[command(after_help = MCP_HELP_TEXT)]
    Mcp,
    /// Print version information
    #[command(visible_alias = "v")]
    Version,
}

async fn run_claude_task(config: TaskRunConfig<'_>) -> Result<()> {
    match config.task_runner {
        ExecutionEnvironment::Docker => run_docker_task(config).await,
        ExecutionEnvironment::Kubernetes => run_kube_task(config).await,
    }
}

async fn validate_kubernetes_access(context: &str) -> Result<()> {
    // Check if kubectl is available
    let kubectl_check = Command::new("kubectl")
        .arg("version")
        .arg("--client")
        .output();

    if kubectl_check.is_err() || !kubectl_check.unwrap().status.success() {
        return Err(anyhow::anyhow!("kubectl is not installed or not in PATH"));
    }

    // Check if the context exists and is accessible
    let context_check = Command::new("kubectl")
        .args(["config", "use-context", context])
        .output()
        .context("Failed to run kubectl")?;

    if !context_check.status.success() {
        let stderr = String::from_utf8_lossy(&context_check.stderr);
        return Err(anyhow::anyhow!(
            "Failed to use context '{}': {}",
            context,
            stderr
        ));
    }

    // Check if we can list jobs (basic permission check)
    let permission_check = Command::new("kubectl")
        .args(["auth", "can-i", "create", "jobs"])
        .output()
        .context("Failed to check permissions")?;

    if !permission_check.status.success() {
        return Err(anyhow::anyhow!(
            "You don't have permission to create jobs in the current context"
        ));
    }

    Ok(())
}

fn get_git_remote_url(path: &Path) -> Result<String> {
    let output = Command::new("git")
        .args(["config", "--get", "remote.origin.url"])
        .current_dir(path)
        .output()
        .context("Failed to get git remote URL")?;

    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "No git remote found. Please ensure this is a git repository with a remote."
        ));
    }

    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if url.is_empty() {
        return Err(anyhow::anyhow!("Git remote URL is empty"));
    }

    Ok(url)
}

/// Get GitHub token from environment or gh CLI
fn get_github_token() -> Option<String> {
    // First try environment variable
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        return Some(token);
    }

    // Try to get from gh CLI
    match Command::new("gh").args(["auth", "token"]).output() {
        Ok(output) if output.status.success() => {
            let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !token.is_empty() {
                return Some(token);
            }
        }
        _ => {}
    }

    None
}

async fn run_kube_task(config: TaskRunConfig<'_>) -> Result<()> {
    let kube_config = config.kube_config.as_ref().ok_or_else(|| {
        anyhow::anyhow!("Kubernetes execution environment requires a kube_config")
    })?;

    // Determine context and namespace (similar logic to setup)
    let context = kube_config
        .context
        .clone()
        .or_else(config::Config::get_current_kube_context)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No Kubernetes context specified and could not detect current context. \
        Please specify a context in config.json or ensure kubectl is configured."
            )
        })?;

    let namespace = kube_config.namespace.clone().unwrap_or_else(|| {
        let suffix = config::Config::generate_namespace_suffix();
        format!("claude-task-{suffix}")
    });

    let task_id = config
        .task_id
        .clone()
        .unwrap_or_else(worktree::generate_short_id);

    println!("Running Claude task in Kubernetes with ID: {task_id}");

    // Validate Kubernetes connectivity
    println!("🔍 Checking Kubernetes cluster connectivity...");
    if let Err(e) = validate_kubernetes_access(&context).await {
        return Err(anyhow::anyhow!("Failed to connect to Kubernetes cluster: {}\n\nPlease ensure:\n1. kubectl is installed\n2. You have a valid kubeconfig\n3. The context '{}' exists\n4. You have permissions to create jobs in namespace '{}'", 
            e, context, namespace));
    }

    // Check if this is an auto-generated namespace and show confirmation if needed
    let needs_confirmation = kube_config.context.is_none() || kube_config.namespace.is_none();
    if needs_confirmation && !kube_config.namespace_confirmed {
        use dialoguer::Confirm;

        println!("🚀 Kubernetes Task Confirmation");
        println!();
        println!("This task will run in:");
        println!("   Context: {context}");
        println!("   Namespace: {namespace} (will be created if it doesn't exist)");
        println!();
        println!("⚠️  Please ensure you have appropriate permissions in this cluster.");
        println!();

        let confirmed = Confirm::new()
            .with_prompt("Do you want to proceed with this task?")
            .default(false)
            .interact()?;

        if !confirmed {
            println!("Task cancelled.");
            return Ok(());
        }

        // Update the config to remember this confirmation
        let config_path = Config::default_config_path();
        let mut full_config = Config::load(Some(&config_path))?;
        if let Some(ref mut kc) = full_config.kube_config {
            kc.context = Some(context.clone());
            kc.namespace = Some(namespace.clone());
            kc.namespace_confirmed = true;
        }
        full_config.save(&config_path)?;
        println!("✓ Configuration saved");
        println!();
    }

    // Create Kubernetes client and ensure namespace exists
    println!("🔧 Creating Kubernetes runner...");
    let k8s_runner = kube::KubernetesJobRunner::new()
        .await
        .context("Failed to connect to Kubernetes cluster")?;

    // Ensure namespace exists before any operations
    println!("📁 Ensuring namespace '{namespace}' exists...");
    k8s_runner.create_namespace(&namespace).await?;

    // Ensure image pull secret exists for GHCR images
    if kube_config.image.contains("ghcr.io") {
        let pull_secret_name = kube_config
            .image_pull_secret
            .clone()
            .unwrap_or_else(|| "ghcr-pull-secret".to_string());

        println!("🔐 Ensuring image pull secret '{pull_secret_name}' exists for GHCR...");

        // Check if we have GitHub token from environment or gh CLI
        if let Some(github_token) = get_github_token() {
            // Try to get username from GITHUB_USERNAME or fall back to system username
            let github_username = std::env::var("GITHUB_USERNAME")
                .or_else(|_| std::env::var("USER"))
                .unwrap_or_else(|_| "unknown".to_string());

            println!("   Using GitHub username: {github_username}");
            println!(
                "   Token source: {}",
                if std::env::var("GITHUB_TOKEN").is_ok() {
                    "GITHUB_TOKEN env var"
                } else {
                    "gh CLI"
                }
            );
            k8s_runner
                .create_docker_registry_secret(
                    &namespace,
                    &pull_secret_name,
                    "ghcr.io",
                    &github_username,
                    &github_token,
                )
                .await?;
        } else {
            println!("   ⚠️  No GitHub token found");
            println!("   To pull from GHCR, you need to either:");
            println!("   1. Set environment variable: export GITHUB_TOKEN=your-token");
            println!("   2. Login with gh CLI: gh auth login");
            println!(
                "   Or create the secret manually: kubectl create secret docker-registry {pull_secret_name} \\"
            );
            println!("     --docker-server=ghcr.io \\");
            println!("     --docker-username=YOUR_GITHUB_USERNAME \\");
            println!("     --docker-password=YOUR_GITHUB_TOKEN \\");
            println!("     -n {namespace}");
        }

        // Also update the saved config if it wasn't set
        if kube_config.image_pull_secret.is_none() {
            let config_path = Config::default_config_path();
            let mut full_config = Config::load(Some(&config_path))?;
            if let Some(ref mut kc) = full_config.kube_config {
                kc.image_pull_secret = Some(pull_secret_name.clone());
            }
            full_config.save(&config_path)?;
            println!("   ✓ Updated config with image pull secret name");
        }
    }

    // Ensure git credentials secret exists
    println!(
        "🔑 Ensuring git credentials secret '{}' exists...",
        kube_config.git_secret_name
    );
    if let Some(github_token) = get_github_token() {
        println!(
            "   Token source: {}",
            if std::env::var("GITHUB_TOKEN").is_ok() {
                "GITHUB_TOKEN env var"
            } else {
                "gh CLI"
            }
        );
        k8s_runner
            .create_git_secret(
                &namespace,
                &kube_config.git_secret_name,
                &kube_config.git_secret_key,
                &github_token,
            )
            .await?;
    } else {
        println!("   ⚠️  No GitHub token found to create git credentials secret");
        println!("   The job may fail if the repository is private.");
        println!("   To provide credentials:");
        println!("   1. Set GITHUB_TOKEN environment variable");
        println!("   2. Login with gh CLI: gh auth login");
        println!("   3. Create the secret manually:");
        println!(
            "      kubectl create secret generic {} \\",
            kube_config.git_secret_name
        );
        println!(
            "        --from-literal={}=YOUR_GITHUB_TOKEN \\",
            kube_config.git_secret_key
        );
        println!("        -n {namespace}");
    }

    // Note features not available in K8s mode
    if config.workspace_dir.is_some() {
        println!("⚠️  Note: Custom workspace directory is not supported in Kubernetes mode");
    }
    if config.ht_mcp_port.is_some() || config.web_view_proxy_port.is_some() {
        println!("⚠️  Note: Port forwarding is not supported in Kubernetes mode");
    }
    if config.open_editor {
        println!("⚠️  Note: Opening editor is not supported in Kubernetes mode");
    }

    // Get current git repository info
    let current_dir = std::env::current_dir().context("Could not get current directory")?;

    // Get git remote URL
    let git_remote_url = get_git_remote_url(&current_dir)?;

    // Check if it's a private repository
    if git_remote_url.contains("github.com") && !git_remote_url.contains("github.com/") {
        println!("⚠️  Note: Your repository appears to be private.");
        println!("   Make sure to create the git credentials secret as shown above.");
    }

    // Generate branch name similar to worktree mode
    let branch_name = format!("{}{}", config.branch_prefix, task_id);

    // Prepare approval tool permission
    let approval_permission = config.approval_tool_permission.clone();

    // Always use the configured git credentials secret
    let secret_name = config
        .git_secret_name
        .as_ref()
        .unwrap_or(&kube_config.git_secret_name);
    let secret_key = config
        .git_secret_key
        .as_ref()
        .unwrap_or(&kube_config.git_secret_key);

    // Check if git credentials secret exists
    println!("🔍 Checking for git credentials secret '{secret_name}'...");

    // The secret should have been created during setup
    // JobConfig will validate it exists before running

    // Determine the image pull secret to use
    let image_pull_secret = if kube_config.image.contains("ghcr.io") {
        kube_config
            .image_pull_secret
            .clone()
            .or_else(|| Some("ghcr-pull-secret".to_string()))
    } else {
        kube_config.image_pull_secret.clone()
    };

    // Create Kubernetes job configuration
    let job_config = kube::JobConfig {
        name: format!("claude-task-{task_id}"),
        namespace: namespace.clone(),
        git_repo: git_remote_url,
        git_branch: Some(branch_name.clone()),
        secret_name: secret_name.clone(),
        secret_key: secret_key.clone(),
        claude_prompt: config.prompt.to_string(),
        claude_permission_tool: approval_permission.clone(),
        claude_mcp_config: config.mcp_config.clone(),
        claude_debug: config.debug,
        claude_skip_permissions: approval_permission.is_none() && config.skip_confirmation,
        image: Some(kube_config.image.clone()),
        image_pull_secret,
        async_mode: config.async_mode,
        timeout_seconds: Some(600), // 10 minutes default
        oauth_token: config.claude_credentials.as_ref().map(|c| c.token.clone()),
    };

    // Run the job
    println!("🚀 Starting Kubernetes Claude task...");
    println!("   Job name: {}", job_config.name);
    println!("   Repository: {}", job_config.git_repo);
    println!(
        "   Branch: {}",
        job_config
            .git_branch
            .as_ref()
            .unwrap_or(&"main".to_string())
    );
    println!("   Namespace: {}", job_config.namespace);
    println!();

    match k8s_runner.run_job(job_config).await {
        Ok(result) => {
            match result {
                kube::JobResult::Sync {
                    stdout,
                    stderr,
                    exit_code,
                } => {
                    // Print the output
                    if !stdout.is_empty() {
                        println!("\n=== JOB OUTPUT ===");
                        println!("{stdout}");
                    }

                    if !stderr.is_empty() {
                        eprintln!("\n=== STDERR ===");
                        eprintln!("{stderr}");
                    }

                    if exit_code == Some(0) {
                        println!("\n✨ Claude task completed successfully in Kubernetes!");
                        println!("   Branch created: {branch_name}");
                        println!(
                            "   You can check out the branch with: git fetch && git checkout {branch_name}"
                        );
                    } else {
                        return Err(anyhow::anyhow!(
                            "Claude task failed with exit code: {:?}",
                            exit_code
                        ));
                    }
                }
                kube::JobResult::Async {
                    job_name,
                    namespace,
                } => {
                    println!("\n✨ Claude task started in Kubernetes!");
                    println!("   Job: {job_name}");
                    println!("   Namespace: {namespace}");
                    println!("   Branch: {branch_name}");
                    // The monitoring commands are already printed by the kube module
                }
            }
        }
        Err(e) => {
            eprintln!("\n❌ Kubernetes job failed: {e:#}");
            return Err(e);
        }
    }

    Ok(())
}

async fn run_docker_task(config: TaskRunConfig<'_>) -> Result<()> {
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
        let mcp_config_path = if std::path::Path::new(&mcp_path).is_absolute() {
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
        Some(tool) => (tool.clone(), false),
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
        None => worktree::generate_short_id(),
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
            custom_dir.clone()
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
                worktree::create_git_worktree(&task_id, "claude-task/", config.worktree_base_dir)?;
            println!("✓ Worktree created: {worktree_path:?} (branch: {branch_name})");

            // Open IDE if requested
            if config.open_editor {
                if let Err(e) = worktree::open_worktree(
                    &worktree_path.to_string_lossy(),
                    config.worktree_config.default_open_command.as_deref(),
                ) {
                    println!("⚠️  Warning: Failed to open IDE: {e}");
                    println!("   Continuing with task execution...");
                }
            }

            worktree_path.to_string_lossy().to_string()
        }
    };
    println!();

    // Create Docker manager
    let docker_manager = DockerManager::new(config.docker_config.clone())
        .context("Failed to create Docker manager")?;

    // Check if home volume exists, run setup if it doesn't
    if config.debug {
        println!(
            "🔍 Checking if {} volume exists...",
            config.docker_config.volumes.home
        );
    }
    let home_volume_exists = docker_manager.check_home_volume_exists().await?;
    if config.debug {
        println!("   Volume exists: {home_volume_exists}");
    }

    if !home_volume_exists {
        println!(
            "🔧 {} volume not found, running setup...",
            config.docker_config.volumes.home
        );

        // Check if we have a token configured
        if let Some(_credentials) = config.claude_credentials {
            // Token-based auth: create minimal setup without credential extraction
            handle_docker_setup(
                config.task_base_home_dir,
                config.debug,
                config.claude_user_config,
                config.claude_credentials,
            )
            .await?;
        } else {
            // Traditional setup with credential extraction
            setup_credentials_and_config(
                config.task_base_home_dir,
                config.debug,
                config.claude_user_config,
            )
            .await?;
        }
        println!();
    } else {
        // Volume exists
        if config.debug {
            println!("✓ {} volume found", config.docker_config.volumes.home);
        }

        // Only sync credentials if not using token auth
        if config.claude_credentials.is_none() {
            if config.debug {
                println!("🔍 Checking credential synchronization...");
            }

            // Create sync manager
            let sync_manager =
                credential_sync::CredentialSyncManager::new(config.task_base_home_dir, &task_id)?;

            // Sync credentials if needed with lock mechanism
            let synced = sync_manager
                .sync_credentials_if_needed(
                    || {
                        let task_base_home_dir = config.task_base_home_dir.to_string();
                        let debug = config.debug;
                        let claude_user_config = config.claude_user_config.clone();

                        async move {
                            // Extract credentials directly
                            let credentials = credentials::extract_keychain_credentials().await?;

                            // Setup the full configuration (including writing the credentials)
                            setup_credentials_and_config_with_cache(
                                &task_base_home_dir,
                                debug,
                                &claude_user_config,
                                true,
                            )
                            .await?;

                            // Return the credentials string for hashing
                            Ok(credentials)
                        }
                    },
                    config.debug,
                )
                .await?;

            if synced {
                println!("🔄 Credentials synchronized successfully");
                println!();
            } else if config.debug {
                println!("✓ Credentials recently validated, skipping sync");
            }
        } else if config.debug {
            println!("✓ Using token authentication, skipping credential sync");
        }
    }

    // Validate ht-mcp availability when web view port or ht-mcp port is requested
    let ht_mcp_available = config::Config::check_ht_mcp_availability();

    if config.ht_mcp_port.is_some() && !ht_mcp_available {
        if config.require_ht_mcp {
            anyhow::bail!(
                "HT-MCP port specified but ht-mcp binary is not available, and require_ht_mcp is set to true. \
                Please install ht-mcp or disable require_ht_mcp in config."
            );
        } else {
            println!("⚠️  HT-MCP port specified but ht-mcp binary is not available");
            println!("   Continuing without ht-mcp functionality (require_ht_mcp=false)");
        }
    }

    if config.web_view_proxy_port.is_some() && (config.ht_mcp_port.is_none() || !ht_mcp_available) {
        println!("ℹ️  Web view proxy port specified but ht-mcp is not properly configured");
        println!("   Web view functionality requires ht-mcp for terminal monitoring");

        if config.require_ht_mcp {
            anyhow::bail!(
                "Web view proxy port requires ht-mcp to be available and enabled, but require_ht_mcp is set to true. \
                Please install ht-mcp and provide --ht-mcp-port or disable require_ht_mcp in config."
            );
        } else {
            println!("   Continuing without web view monitoring (require_ht_mcp=false)");
        }
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
        if let Some(port) = claude_config.web_view_proxy_port {
            println!("   - Web view proxy port (host): {port}");
        } else {
            println!("   - Web view proxy port: disabled");
        }
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
            .check_image_exists(&config.docker_config.image_name)
            .await
            .is_err()
        {
            println!("⚠️  Image '{}' not found.", config.docker_config.image_name);
            println!("   Use '--build' flag to build the image first, or build it manually:");
            println!(
                "   docker build -t {} ./claude-task/",
                config.docker_config.image_name
            );
            return Err(anyhow::anyhow!(
                "Image '{}' not found. Use --build flag to build it.",
                config.docker_config.image_name
            ));
        }
        println!(
            "✓ Using existing image: {}",
            config.docker_config.image_name
        );
    }

    // Run Claude task
    let run_options = docker::RunTaskOptions {
        prompt: config.prompt.to_string(),
        permission_prompt_tool: permission_tool_arg,
        debug: config.debug,
        mcp_config: validated_mcp_config.clone(),
        skip_permissions,
        async_mode: config.async_mode,
        oauth_token: config.claude_credentials.as_ref().map(|c| c.token.clone()),
    };

    let result = docker_manager
        .run_claude_task(&claude_config, &run_options)
        .await;

    match result {
        Ok(docker::TaskRunResult::Sync { output }) => {
            // Output was already streamed during execution
            let _ = output;

            // Update validation timestamp on successful completion
            let sync_manager =
                credential_sync::CredentialSyncManager::new(config.task_base_home_dir, &task_id)?;

            if let Err(e) = sync_manager.update_validation_timestamp() {
                if config.debug {
                    println!("⚠️  Warning: Failed to update validation timestamp: {e}");
                }
            }

            println!("✅ Claude task completed successfully!");
            println!("   Task ID: {task_id}");
            println!("   Shared volume: {}", config.docker_config.volumes.home);
        }
        Ok(docker::TaskRunResult::Async {
            task_id: async_task_id,
            container_id,
        }) => {
            // Task is running in background
            let _ = async_task_id;
            println!("\n📋 Task is running in background");
            println!("   Container ID: {container_id}");
            println!("   Monitor logs: docker logs -f {container_id}");
            println!("   Stop task: docker stop {container_id}");
            println!("   Clean up: docker rm {container_id}");

            // Note: For async tasks, we cannot update validation timestamp
            // as we don't know when/if they complete successfully
        }
        Err(e) => {
            // Check if this is a credential error
            let error_msg = e.to_string();
            if credential_sync::CredentialSyncManager::is_credential_error(&error_msg) {
                println!("🔐 Credential error detected: {e}");
                println!("🔄 Attempting to refresh credentials and retry...");

                // Force credential sync
                let sync_manager = credential_sync::CredentialSyncManager::new(
                    config.task_base_home_dir,
                    &task_id,
                )?;

                sync_manager
                    .sync_credentials_if_needed(
                        || {
                            let task_base_home_dir = config.task_base_home_dir.to_string();
                            let debug = config.debug;
                            let claude_user_config = config.claude_user_config.clone();

                            async move {
                                // Extract credentials directly
                                let credentials =
                                    credentials::extract_keychain_credentials().await?;

                                // Setup the full configuration (including writing the credentials)
                                setup_credentials_and_config_with_cache(
                                    &task_base_home_dir,
                                    debug,
                                    &claude_user_config,
                                    true,
                                )
                                .await?;

                                // Return the credentials string for hashing
                                Ok(credentials)
                            }
                        },
                        config.debug,
                    )
                    .await?;

                // Retry the task once
                println!("🔄 Retrying task with refreshed credentials...");
                let retry_result = docker_manager
                    .run_claude_task(&claude_config, &run_options)
                    .await?;

                match retry_result {
                    docker::TaskRunResult::Sync { output } => {
                        let _ = output;

                        // Update validation timestamp on successful retry
                        if let Err(e) = sync_manager.update_validation_timestamp() {
                            if config.debug {
                                println!("⚠️  Warning: Failed to update validation timestamp: {e}");
                            }
                        }

                        println!("✅ Claude task completed successfully after retry!");
                        println!("   Task ID: {task_id}");
                        println!("   Shared volume: {}", config.docker_config.volumes.home);
                    }
                    docker::TaskRunResult::Async {
                        task_id: async_task_id,
                        container_id,
                    } => {
                        let _ = async_task_id;
                        println!("\n📋 Task is running in background (after retry)");
                        println!("   Container ID: {container_id}");
                        println!("   Monitor logs: docker logs -f {container_id}");
                        println!("   Stop task: docker stop {container_id}");
                        println!("   Clean up: docker rm {container_id}");
                    }
                }
            } else {
                // Not a credential error, propagate it
                return Err(e);
            }
        }
    }

    Ok(())
}

async fn clean_all_worktrees_and_volumes(
    branch_prefix: &str,
    skip_confirmation: bool,
    force: bool,
    _docker_config: &config::DockerConfig,
    auto_clean_branch: bool,
) -> Result<()> {
    println!("🧹 Cleaning up all claude-task git worktrees and Docker volumes...");

    // Clean worktrees
    worktree::clean_all_worktrees(branch_prefix, skip_confirmation, force, auto_clean_branch)
        .await?;

    // Clean Docker volumes TODO: add this back in
    // docker::clean_shared_volumes(false, docker_config).await?;

    println!("\n✅ All clean up operations completed.");
    Ok(())
}

async fn handle_docker_setup(
    task_base_home_dir: &str,
    debug: bool,
    claude_user_config: &config::ClaudeUserConfig,
    claude_credentials: &Option<config::ClaudeCredentials>,
) -> Result<()> {
    // Check if we have a token in config
    if let Some(_credentials) = claude_credentials {
        println!("🔑 Using long-lived token from config...");
        println!("   Token configured in claudeCredentials.token");
        println!("   This token will be injected as CLAUDE_CODE_OAUTH_TOKEN");
        println!();
        println!("ℹ️  To generate a new token, run: claude setup-token");
        println!("   Then add it to your config.json under claudeCredentials.token");

        // Create minimal directory structure for token auth
        let base_dir = Config::expand_tilde(task_base_home_dir);
        let claude_dir = base_dir.join(".claude");
        std::fs::create_dir_all(&claude_dir)?;

        // Create empty credentials file that might be expected
        std::fs::write(claude_dir.join(".credentials.json"), "{}")?;

        // Copy user memory if it exists
        let user_memory_path = Config::expand_tilde(&claude_user_config.user_memory_path);
        if user_memory_path.exists() {
            let dest_path = claude_dir.join("CLAUDE.md");
            std::fs::copy(&user_memory_path, &dest_path)?;
            println!("✓ Copied user memory to {}", dest_path.display());
        } else {
            // Create default CLAUDE.md
            let claude_md_content = assets::get_claude_md_content();
            std::fs::write(claude_dir.join("CLAUDE.md"), claude_md_content)?;
            println!("✓ Created default CLAUDE.md");
        }

        // Create minimal claude config
        let claude_config_path = base_dir.join(".claude.json");
        std::fs::write(&claude_config_path, "{}")?;

        // Create Docker home volume with bind mount
        println!("Creating Docker volume 'claude-task-home'...");
        credentials::create_docker_home_volume_only(&base_dir.to_string_lossy()).await?;

        println!("✓ Token-based setup completed");
    } else {
        // This is the existing setup logic for Docker
        setup_credentials_and_config(task_base_home_dir, debug, claude_user_config).await?;
    }
    Ok(())
}

async fn handle_kubernetes_setup(
    task_base_home_dir: &str,
    debug: bool,
    claude_user_config: &config::ClaudeUserConfig,
    claude_credentials: &Option<config::ClaudeCredentials>,
    kube_config: &Option<config::KubeConfig>,
) -> Result<()> {
    use dialoguer::Confirm;

    // Check if Kubernetes is configured
    let mut kube_config = kube_config
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Kubernetes configuration not found in config.json"))?
        .clone();

    // Determine context and namespace
    let (final_context, final_namespace, needs_confirmation) =
        if kube_config.context.is_none() || kube_config.namespace.is_none() {
            // Detect current context if not specified
            let context = kube_config
                .context
                .clone()
                .or_else(config::Config::get_current_kube_context)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "No Kubernetes context specified and could not detect current context. \
            Please specify a context in config.json or ensure kubectl is configured."
                    )
                })?;

            // Generate namespace if not specified
            let namespace = kube_config.namespace.clone().unwrap_or_else(|| {
                let suffix = config::Config::generate_namespace_suffix();
                format!("claude-task-{suffix}")
            });

            (context, namespace, !kube_config.namespace_confirmed)
        } else {
            // Both context and namespace are specified, user knows what they're doing
            (
                kube_config.context.clone().unwrap(),
                kube_config.namespace.clone().unwrap(),
                false,
            )
        };

    // Show confirmation if needed
    if needs_confirmation {
        println!("🚀 Kubernetes Setup Confirmation");
        println!();
        println!("This will create Kubernetes resources in:");
        println!("   Context: {final_context}");
        println!("   Namespace: {final_namespace} (will be created if it doesn't exist)");
        println!();
        println!("The following resources will be created:");
        println!("   - Namespace (if needed)");
        println!("   - Secrets for Git and Claude credentials");
        println!("   - Image pull secret for ghcr.io");
        println!();
        println!("⚠️  Please ensure you have appropriate permissions in this cluster.");
        println!();

        let confirmed = Confirm::new()
            .with_prompt("Do you want to proceed with this setup?")
            .default(false)
            .interact()?;

        if !confirmed {
            println!("Setup cancelled.");
            return Ok(());
        }

        // Update the config to remember this confirmation
        kube_config.namespace_confirmed = true;

        // Save the updated config
        let config_path = Config::default_config_path();
        let mut full_config = Config::load(Some(&config_path))?;
        if let Some(ref mut kc) = full_config.kube_config {
            kc.context = Some(final_context.clone());
            kc.namespace = Some(final_namespace.clone());
            kc.namespace_confirmed = true;
        }
        full_config.save(&config_path)?;
        println!("✓ Configuration saved");
    }

    println!("🚀 Setting up Kubernetes environment...");
    println!("   Context: {final_context}");
    println!("   Namespace: {final_namespace}");
    println!();

    // First, ensure credentials are available (either token or extracted)
    println!("📋 Ensuring Claude credentials are available...");
    let home_volume_path = Config::expand_tilde(task_base_home_dir);

    if let Some(_credentials) = claude_credentials {
        println!("   ✓ Using long-lived token from config");

        // Ensure minimal setup for token auth
        handle_docker_setup(
            task_base_home_dir,
            debug,
            claude_user_config,
            claude_credentials,
        )
        .await?;
    } else {
        // Check if credentials exist from previous Docker setup
        let credentials_path = home_volume_path.join(".claude/.credentials.json");
        let config_path = home_volume_path.join(".claude.json");

        if !credentials_path.exists() || !config_path.exists() {
            println!("   ⚠️  Claude credentials not found. Running Docker setup first...");
            println!();
            setup_credentials_and_config_with_cache(
                task_base_home_dir,
                debug,
                claude_user_config,
                false, // don't update cache
            )
            .await?;
        } else {
            println!("   ✓ Claude credentials found");
        }
    }

    // Create Kubernetes client
    println!("\n🔧 Connecting to Kubernetes cluster...");
    let k8s_runner = kube::KubernetesJobRunner::new()
        .await
        .context("Failed to connect to Kubernetes cluster")?;

    // Ensure namespace exists
    println!("\n📁 Ensuring namespace '{final_namespace}' exists...");
    k8s_runner.create_namespace(&final_namespace).await?;

    // Create ghcr-pull-secret if it doesn't exist
    if let Some(ref pull_secret_name) = kube_config.image_pull_secret {
        println!("\n🔐 Setting up image pull secret '{pull_secret_name}'...");

        // Check if we have GitHub token from environment or gh CLI
        if let Some(github_token) = get_github_token() {
            // Try to get username from GITHUB_USERNAME or fall back to system username
            let github_username = std::env::var("GITHUB_USERNAME")
                .or_else(|_| std::env::var("USER"))
                .unwrap_or_else(|_| "unknown".to_string());

            println!("   Using GitHub username: {github_username}");
            println!(
                "   Token source: {}",
                if std::env::var("GITHUB_TOKEN").is_ok() {
                    "GITHUB_TOKEN env var"
                } else {
                    "gh CLI"
                }
            );
            k8s_runner
                .create_docker_registry_secret(
                    &final_namespace,
                    pull_secret_name,
                    "ghcr.io",
                    &github_username,
                    &github_token,
                )
                .await?;
        } else {
            println!(
                "   ⚠️  Please ensure '{pull_secret_name}' secret exists for pulling images from GHCR"
            );
            println!("   Create with: kubectl create secret docker-registry {pull_secret_name} \\");
            println!("     --docker-server=ghcr.io \\");
            println!("     --docker-username=YOUR_GITHUB_USERNAME \\");
            println!("     --docker-password=YOUR_GITHUB_TOKEN \\");
            println!("     -n {final_namespace}");
        }
    }

    // Create git credentials secret
    println!("\n🔑 Git credentials secret...");
    println!("   Secret name: {}", kube_config.git_secret_name);
    println!("   Secret key: {}", kube_config.git_secret_key);

    // Check if we have GitHub token from environment or gh CLI for git secret
    if let Some(github_token) = get_github_token() {
        println!(
            "   Token source: {}",
            if std::env::var("GITHUB_TOKEN").is_ok() {
                "GITHUB_TOKEN env var"
            } else {
                "gh CLI"
            }
        );
        k8s_runner
            .create_git_secret(
                &final_namespace,
                &kube_config.git_secret_name,
                &kube_config.git_secret_key,
                &github_token,
            )
            .await?;
    } else {
        println!("   ⚠️  Git tokens should be provided via --git-token flag when running tasks");
        println!("   Or create the secret manually:");
        println!(
            "   kubectl create secret generic {} \\",
            kube_config.git_secret_name
        );
        println!(
            "     --from-literal={}=YOUR_GITHUB_TOKEN \\",
            kube_config.git_secret_key
        );
        println!("     -n {final_namespace}");
    }

    // Create Claude credentials secret
    println!("\n📦 Creating Claude credentials secret...");
    let secret_name = "claude-credentials";

    k8s_runner
        .create_claude_credentials_secret(&final_namespace, secret_name, &home_volume_path)
        .await?;

    println!("\n✅ Kubernetes setup completed!");
    println!("\nVerify your secrets with:");
    println!("   kubectl get secrets -n {final_namespace}");

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Handle config command first as it doesn't need the config loaded
    if let Some(Commands::Config { command }) = &cli.command {
        handle_config_command(command.clone(), cli.config_path.as_ref()).await?;
        return Ok(());
    }

    // Load configuration for other commands
    let config = Config::load(cli.config_path.as_ref())?;

    // Override config with CLI args if provided
    let debug = if cli.debug {
        true
    } else {
        config.global_option_defaults.debug
    };
    let require_ht_mcp = if cli.require_ht_mcp {
        true
    } else {
        config.global_option_defaults.require_ht_mcp
    };

    match cli.command {
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
            async_mode,
            execution_env,
            kube_namespace,
            kube_context,
            git_secret_name,
            git_secret_key,
        }) => {
            // Override execution environment if specified
            let exec_env = execution_env.as_ref().unwrap_or(&config.task_runner);

            // Override kubernetes config if needed
            let mut kube_config_override = config.kube_config.clone();
            if exec_env == &ExecutionEnvironment::Kubernetes {
                if let Some(ref mut kube_cfg) = kube_config_override {
                    if let Some(ref namespace) = kube_namespace {
                        kube_cfg.namespace = Some(namespace.clone());
                    }
                    if let Some(ref context) = kube_context {
                        kube_cfg.context = Some(context.clone());
                    }
                } else if kube_namespace.is_some() || kube_context.is_some() {
                    // Create a default kube config if CLI args are provided but config is missing
                    kube_config_override = Some(config::KubeConfig {
                        namespace: kube_namespace.clone(),
                        context: kube_context.clone(),
                        image: "ghcr.io/onegrep/claude-task:latest".to_string(),
                        git_secret_name: "git-credentials".to_string(),
                        git_secret_key: "token".to_string(),
                        image_pull_secret: Some("ghcr-pull-secret".to_string()),
                        namespace_confirmed: false,
                    });
                }
            }

            let task_config = TaskRunConfig {
                prompt: &prompt,
                task_id: task_id.clone(),
                build,
                workspace_dir: workspace_dir.clone(),
                approval_tool_permission: approval_tool_permission.clone(),
                debug,
                mcp_config: mcp_config.clone(),
                skip_confirmation: yes,
                worktree_base_dir: &config.paths.worktree_base_dir,
                task_base_home_dir: &config.paths.task_base_home_dir,
                branch_prefix: &config.paths.branch_prefix,
                open_editor,
                ht_mcp_port,
                web_view_proxy_port,
                require_ht_mcp,
                docker_config: &config.docker,
                claude_user_config: &config.claude_user_config,
                worktree_config: &config.worktree,
                async_mode,
                task_runner: exec_env,
                kube_config: &kube_config_override,
                git_secret_name: git_secret_name.clone(),
                git_secret_key: git_secret_key.clone(),
                claude_credentials: &config.claude_credentials,
            };

            if let Err(e) = run_claude_task(task_config).await {
                eprintln!("❌ Error running task: {e:#?}");
                // Print the full error chain
                let mut source = e.source();
                while let Some(err) = source {
                    eprintln!("Caused by: {err}");
                    source = err.source();
                }
                std::process::exit(1);
            }
        }
        Some(Commands::Worktree { command }) => match command {
            WorktreeCommands::Create { task_id } => {
                worktree::create_git_worktree(
                    &task_id,
                    &config.paths.branch_prefix,
                    &config.paths.worktree_base_dir,
                )?;
            }
            WorktreeCommands::List => {
                worktree::list_git_worktrees(&config.paths.branch_prefix)?;
            }
            WorktreeCommands::Remove { task_id } => {
                worktree::remove_git_worktree(
                    &task_id,
                    &config.paths.branch_prefix,
                    config.worktree.auto_clean_on_remove,
                )?;
            }
            WorktreeCommands::Open => {
                worktree::select_worktree_interactively(
                    &config.paths.branch_prefix,
                    config.worktree.default_open_command.as_deref(),
                )?;
            }
            WorktreeCommands::Clean { yes, force } => {
                worktree::clean_all_worktrees(
                    &config.paths.branch_prefix,
                    yes,
                    force,
                    config.worktree.auto_clean_on_remove,
                )
                .await?;
            }
        },
        Some(Commands::Docker { command }) => match command {
            DockerCommands::Init {
                refresh_credentials: _,
            } => {
                // init_shared_volumes(
                //     refresh_credentials,
                //     &config.paths.task_base_home_dir,
                //     debug,
                //     &config.docker,
                //     &config.claude_user_config,
                // )
                // .await?;
            }
            DockerCommands::List => {
                // list_docker_volumes(&config.docker).await?;
            }
            DockerCommands::Clean => {
                // clean_shared_volumes(debug, &config.docker).await?;
            }
        },
        Some(Commands::Config { .. }) => {
            // Already handled above
            unreachable!("Config command should have been handled earlier");
        }
        Some(Commands::Setup { command }) => match command {
            SetupCommands::Docker => {
                handle_docker_setup(
                    &config.paths.task_base_home_dir,
                    debug,
                    &config.claude_user_config,
                    &config.claude_credentials,
                )
                .await?;
            }
            SetupCommands::Kubernetes => {
                handle_kubernetes_setup(
                    &config.paths.task_base_home_dir,
                    debug,
                    &config.claude_user_config,
                    &config.claude_credentials,
                    &config.kube_config,
                )
                .await?;
            }
        },
        Some(Commands::Clean { yes, force }) => {
            clean_all_worktrees_and_volumes(
                &config.paths.branch_prefix,
                yes,
                force,
                &config.docker,
                config.worktree.auto_clean_on_remove,
            )
            .await?;
        }
        Some(Commands::Mcp) => {
            mcp::run_mcp_server().await?;
        }
        Some(Commands::Version) => {
            println!("claude-task version: {}", config.version);
        }
        None => {
            // No subcommand, print help
            Cli::command().print_help()?;
        }
    }

    Ok(())
}
