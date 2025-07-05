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
use crate::{
    clean_all_worktrees_and_volumes, clean_shared_volumes, create_git_worktree,
    init_shared_volumes, remove_git_worktree, run_claude_task, setup_credentials_and_config,
    TaskRunConfig,
};

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
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CleanOptions {
    #[serde(flatten)]
    pub global_options: GlobalOptions,
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
        let task_base_home_dir = args
            .global_options
            .task_base_home_dir
            .unwrap_or_else(|| "~/.claude-task/home".to_string());
        let debug = args.global_options.debug.unwrap_or(false);

        setup_credentials_and_config(&task_base_home_dir, debug)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(
            "Setup completed successfully".to_string(),
        )]))
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

    #[tool(description = "Initialize shared Docker volumes for Claude tasks")]
    async fn init_docker_volume(
        &self,
        Parameters(args): Parameters<InitDockerVolumeOptions>,
    ) -> Result<CallToolResult, McpError> {
        let task_base_home_dir = args
            .global_options
            .task_base_home_dir
            .unwrap_or_else(|| "~/.claude-task/home".to_string());
        let debug = args.global_options.debug.unwrap_or(false);
        let refresh_credentials = args.refresh_credentials.unwrap_or(false);

        init_shared_volumes(refresh_credentials, &task_base_home_dir, debug)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(
            "All shared volumes are ready".to_string(),
        )]))
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
        let debug = args.global_options.debug.unwrap_or(false);

        clean_shared_volumes(debug)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(
            "Shared volume cleanup completed".to_string(),
        )]))
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

        let worktree_base_dir = args
            .global_options
            .worktree_base_dir
            .unwrap_or_else(|| "~/.claude-task/worktrees".to_string());
        let task_base_home_dir = args
            .global_options
            .task_base_home_dir
            .unwrap_or_else(|| "~/.claude-task/home".to_string());

        let config = TaskRunConfig {
            prompt: &args.prompt,
            task_id: args.task_id,
            build: args.build.unwrap_or(false),
            workspace_dir: args.workspace_dir,
            approval_tool_permission: Some(args.approval_tool_permission),
            debug: args.debug.unwrap_or(false),
            mcp_config: args.mcp_config,
            skip_confirmation: true, // Skip confirmation in MCP mode
            worktree_base_dir: &worktree_base_dir,
            task_base_home_dir: &task_base_home_dir,
            open_editor: false, // Don't auto-open IDE in MCP mode
            ht_mcp_port: None,  // HT-MCP port not supported via MCP interface
        };

        run_claude_task(config)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(
            "Claude task completed successfully".to_string(),
        )]))
    }

    #[tool(description = "Clean up all claude-task git worktrees and docker volumes")]
    async fn clean(
        &self,
        Parameters(args): Parameters<CleanOptions>,
    ) -> Result<CallToolResult, McpError> {
        let branch_prefix = args
            .global_options
            .branch_prefix
            .unwrap_or_else(|| "claude-task/".to_string());

        // Always skip confirmation since we're deferring to the permission tool to approve or reject
        clean_all_worktrees_and_volumes(&branch_prefix, true)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(
            "Cleanup completed successfully".to_string(),
        )]))
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
