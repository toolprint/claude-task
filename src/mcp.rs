use anyhow::Result;
use rmcp::{
    handler::server::{router::tool::ToolRouter, tool::Parameters},
    model::*,
    schemars, tool, tool_handler, tool_router,
    transport::stdio,
    Error as McpError, ServerHandler, ServiceExt,
};
use serde::{Deserialize, Serialize};
use std::future::Future;
use tracing_subscriber::{self, EnvFilter};

use claude_task::permission::ApprovalToolPermission;

// Import internal functions from the main module
use crate::{check_worktree_status, clean_all_worktrees, create_git_worktree, remove_git_worktree};

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GlobalOptions {
    pub worktree_base_dir: Option<String>,
    pub branch_prefix: Option<String>,
    pub task_base_home_dir: Option<String>,
    pub debug: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SetupOptions {
    #[serde(flatten)]
    pub global_options: GlobalOptions,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CreateWorktreeOptions {
    #[serde(flatten)]
    pub global_options: GlobalOptions,
    pub task_id: String,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ListWorktreeOptions {
    #[serde(flatten)]
    pub global_options: GlobalOptions,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RemoveWorktreeOptions {
    #[serde(flatten)]
    pub global_options: GlobalOptions,
    pub task_id: String,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CleanWorktreeOptions {
    #[serde(flatten)]
    pub global_options: GlobalOptions,
    pub force: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct InitDockerVolumeOptions {
    #[serde(flatten)]
    pub global_options: GlobalOptions,
    pub refresh_credentials: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ListDockerVolumeOptions {
    #[serde(flatten)]
    pub global_options: GlobalOptions,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CleanDockerVolumeOptions {
    #[serde(flatten)]
    pub global_options: GlobalOptions,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RunTaskOptions {
    #[serde(flatten)]
    pub global_options: GlobalOptions,
    pub prompt: String,
    pub task_id: Option<String>,
    pub build: Option<bool>,
    pub workspace_dir: Option<Option<String>>,
    pub approval_tool_permission: String,
    pub debug: Option<bool>,
    pub mcp_config: Option<String>,
    pub web_view_proxy_port: Option<u16>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CleanOptions {
    #[serde(flatten)]
    pub global_options: GlobalOptions,
    pub force: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CheckWorktreeStatusOptions {
    #[serde(flatten)]
    pub global_options: GlobalOptions,
    pub worktree_path: Option<String>,
}

// Individual tool input structs for each subcommand use the Options structs directly

#[derive(Clone)]
pub struct ClaudeTaskMcpServer {
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl ClaudeTaskMcpServer {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Setup claude-task with your current environment")]
    async fn setup(
        &self,
        Parameters(args): Parameters<SetupOptions>,
    ) -> Result<CallToolResult, McpError> {
        // Use subprocess since we need claude user config which isn't available in MCP context
        let mut cmd_args = vec!["setup".to_string()];
        self.add_global_options(&mut cmd_args, &args.global_options);

        let output = self
            .execute_claude_task_command(&cmd_args)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    #[tool(description = "Create a git worktree for a task")]
    async fn create_worktree(
        &self,
        Parameters(args): Parameters<CreateWorktreeOptions>,
    ) -> Result<CallToolResult, McpError> {
        let branch_prefix = args
            .global_options
            .branch_prefix
            .unwrap_or_else(|| "claude-task/".to_string());
        let worktree_base_dir = args
            .global_options
            .worktree_base_dir
            .unwrap_or_else(|| "~/.claude-task/worktrees".to_string());

        let (worktree_path, branch_name) =
            create_git_worktree(&args.task_id, &branch_prefix, &worktree_base_dir)
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let output = format!(
            "Git worktree created successfully\nBranch: {branch_name}\nPath: {worktree_path:?}"
        );
        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    #[tool(description = "List git worktrees")]
    async fn list_worktree(
        &self,
        Parameters(args): Parameters<ListWorktreeOptions>,
    ) -> Result<CallToolResult, McpError> {
        // We need to capture the output instead of printing directly
        // For now, we'll use the subprocess approach but this could be improved
        // by refactoring list_git_worktrees to return output instead of printing
        let mut cmd_args = vec!["worktree".to_string(), "list".to_string()];
        self.add_global_options(&mut cmd_args, &args.global_options);

        let output = self
            .execute_claude_task_command(&cmd_args)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    #[tool(description = "Remove a git worktree")]
    async fn remove_worktree(
        &self,
        Parameters(args): Parameters<RemoveWorktreeOptions>,
    ) -> Result<CallToolResult, McpError> {
        let branch_prefix = args
            .global_options
            .branch_prefix
            .unwrap_or_else(|| "claude-task/".to_string());

        remove_git_worktree(&args.task_id, &branch_prefix)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let output = format!("Cleanup complete for task '{}'", args.task_id);
        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    #[tool(description = "Clean up all claude-task git worktrees")]
    async fn clean_worktree(
        &self,
        Parameters(args): Parameters<CleanWorktreeOptions>,
    ) -> Result<CallToolResult, McpError> {
        let branch_prefix = args
            .global_options
            .branch_prefix
            .unwrap_or_else(|| "claude-task/".to_string());
        let force = args.force.unwrap_or(false);

        // Always skip confirmation in MCP mode
        clean_all_worktrees(&branch_prefix, true, force)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(
            "Worktree cleanup completed successfully".to_string(),
        )]))
    }

    #[tool(description = "Initialize shared Docker volumes for Claude tasks")]
    async fn init_docker_volume(
        &self,
        Parameters(args): Parameters<InitDockerVolumeOptions>,
    ) -> Result<CallToolResult, McpError> {
        // Use subprocess since we need docker config which isn't available in MCP context
        let mut cmd_args = vec!["docker".to_string(), "init".to_string()];
        if args.refresh_credentials.unwrap_or(false) {
            cmd_args.push("--refresh-credentials".to_string());
        }
        self.add_global_options(&mut cmd_args, &args.global_options);

        let output = self
            .execute_claude_task_command(&cmd_args)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    #[tool(description = "List Docker volumes for Claude tasks")]
    async fn list_docker_volume(
        &self,
        Parameters(args): Parameters<ListDockerVolumeOptions>,
    ) -> Result<CallToolResult, McpError> {
        // For now, use subprocess since list_docker_volumes prints directly
        // This could be improved by refactoring to return output instead of printing
        let mut cmd_args = vec!["docker".to_string(), "list".to_string()];
        self.add_global_options(&mut cmd_args, &args.global_options);

        let output = self
            .execute_claude_task_command(&cmd_args)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    #[tool(description = "Clean up all shared Docker volumes")]
    async fn clean_docker_volume(
        &self,
        Parameters(args): Parameters<CleanDockerVolumeOptions>,
    ) -> Result<CallToolResult, McpError> {
        // Use subprocess since we need docker config which isn't available in MCP context
        let mut cmd_args = vec!["docker".to_string(), "clean".to_string()];
        self.add_global_options(&mut cmd_args, &args.global_options);

        let output = self
            .execute_claude_task_command(&cmd_args)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    #[tool(description = "Run a Claude task in a local docker container")]
    async fn run_task(
        &self,
        Parameters(args): Parameters<RunTaskOptions>,
    ) -> Result<CallToolResult, McpError> {
        // Validate approval tool permission format if not empty
        if let Err(e) = ApprovalToolPermission::parse(&args.approval_tool_permission) {
            return Err(McpError::invalid_params(
                format!(
                    "Invalid approval tool permission format: {e}\n\nExpected format: mcp__<server_name>__<tool_name>\nExample: mcp__approval_server__approve_command"
                ),
                None,
            ));
        }

        // Use subprocess since we need docker config which isn't available in MCP context
        let mut cmd_args = vec!["run".to_string(), args.prompt.clone()];

        if let Some(task_id) = args.task_id {
            cmd_args.push("--task-id".to_string());
            cmd_args.push(task_id);
        }
        if args.build.unwrap_or(false) {
            cmd_args.push("--build".to_string());
        }
        if let Some(ws) = args.workspace_dir {
            cmd_args.push("--workspace-dir".to_string());
            if let Some(dir) = ws {
                cmd_args.push(dir);
            }
        }
        if !args.approval_tool_permission.is_empty() {
            cmd_args.push("-a".to_string());
            cmd_args.push(args.approval_tool_permission.clone());
        }
        if let Some(mcp) = args.mcp_config {
            cmd_args.push("-c".to_string());
            cmd_args.push(mcp);
        }
        cmd_args.push("--yes".to_string()); // Skip confirmation in MCP
        if let Some(port) = args.web_view_proxy_port {
            cmd_args.push("--web-view-proxy-port".to_string());
            cmd_args.push(port.to_string());
        }
        self.add_global_options(&mut cmd_args, &args.global_options);

        let output = self
            .execute_claude_task_command(&cmd_args)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    #[tool(description = "Clean up both claude-task git worktrees and docker volumes")]
    async fn clean(
        &self,
        Parameters(args): Parameters<CleanOptions>,
    ) -> Result<CallToolResult, McpError> {
        let force = args.force.unwrap_or(false);

        // Use subprocess since we need docker config which isn't available in MCP context
        let mut cmd_args = vec!["clean".to_string()];
        if force {
            cmd_args.push("--force".to_string());
        }
        cmd_args.push("--yes".to_string()); // Always skip confirmation in MCP
        self.add_global_options(&mut cmd_args, &args.global_options);

        let output = self
            .execute_claude_task_command(&cmd_args)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    #[tool(description = "Check git worktree status for uncommitted changes and unpushed commits")]
    async fn check_worktree_status(
        &self,
        Parameters(args): Parameters<CheckWorktreeStatusOptions>,
    ) -> Result<CallToolResult, McpError> {
        use std::path::PathBuf;

        let worktree_path = if let Some(path) = args.worktree_path {
            PathBuf::from(path)
        } else {
            std::env::current_dir().map_err(|e| McpError::internal_error(e.to_string(), None))?
        };

        let status = check_worktree_status(&worktree_path)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let details = status.get_status_details();

        let mut output = format!(
            "Status: {}\n",
            if status.is_clean() {
                "✅ Clean"
            } else {
                "⚠️ Unclean"
            }
        );
        output.push_str(&format!("Branch: {}\n", status.current_branch));

        if let Some(remote) = &status.remote_branch {
            output.push_str(&format!("Remote: {remote}\n"));
        }

        if !status.is_clean() {
            output.push_str(&format!("\nIssues: {}\n", details.join(", ")));

            // Show merge info if detected
            if status.is_likely_merged {
                if let Some(ref info) = status.merge_info {
                    output.push_str(&format!(
                        "\nNote: Branch appears to be {info} - remote may have been deleted\n"
                    ));
                }
            }

            // Show changed files if any
            if !status.changed_files.is_empty() {
                output.push_str("\nChanged files:\n");
                for file in &status.changed_files {
                    output.push_str(&format!("  - {file}\n"));
                }
            }

            // Show untracked files if any
            if !status.untracked_files.is_empty() {
                output.push_str("\nUntracked files:\n");
                for file in &status.untracked_files {
                    output.push_str(&format!("  - {file}\n"));
                }
            }

            // Show unpushed commits if any
            if !status.unpushed_commits.is_empty() {
                if status.is_likely_merged {
                    output.push_str("\nCommits (likely already merged):\n");
                } else {
                    output.push_str("\nUnpushed commits:\n");
                }
                for (commit_id, message) in &status.unpushed_commits {
                    output.push_str(&format!("  - {commit_id} {message}\n"));
                }
            }
        } else if status.is_likely_merged {
            if let Some(ref info) = status.merge_info {
                output.push_str(&format!("\nMerge status: {info}\n"));
            }
        }

        if status.behind_count > 0 {
            output.push_str(&format!(
                "\nNote: {} commits behind remote\n",
                status.behind_count
            ));
        }

        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    fn add_global_options(&self, cmd_args: &mut Vec<String>, global_options: &GlobalOptions) {
        if let Some(worktree_base_dir) = &global_options.worktree_base_dir {
            cmd_args.extend(["--worktree-base-dir".to_string(), worktree_base_dir.clone()]);
        }
        if let Some(branch_prefix) = &global_options.branch_prefix {
            cmd_args.extend(["--branch-prefix".to_string(), branch_prefix.clone()]);
        }
        if let Some(task_base_home_dir) = &global_options.task_base_home_dir {
            cmd_args.extend([
                "--task-base-home-dir".to_string(),
                task_base_home_dir.clone(),
            ]);
        }
        if let Some(true) = global_options.debug {
            cmd_args.push("--debug".to_string());
        }
    }

    async fn execute_claude_task_command(&self, args: &[String]) -> Result<String> {
        let output = tokio::process::Command::new("claude-task")
            .args(args)
            .output()
            .await?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(anyhow::anyhow!("Command failed: {}", stderr))
        }
    }
}

#[tool_handler]
impl ServerHandler for ClaudeTaskMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::default(),
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability {
                    list_changed: Some(true),
                }),
                ..Default::default()
            },
            server_info: Implementation {
                name: "claude-task-mcp".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            instructions: Some("MCP server for claude-task CLI tool".to_string()),
        }
    }
}

pub async fn run_mcp_server() -> Result<()> {
    // Initialize the tracing subscriber with file and stdout logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::DEBUG.into()))
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    tracing::info!("Starting claude-task MCP server");

    // Create an instance of our claude-task server
    let service = ClaudeTaskMcpServer::new()
        .serve(stdio())
        .await
        .inspect_err(|e| {
            tracing::error!("serving error: {:?}", e);
        })?;

    service.waiting().await?;
    Ok(())
}
