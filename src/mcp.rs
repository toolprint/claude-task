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
pub struct InitVolumeOptions {
    #[serde(flatten)]
    pub global_options: GlobalOptions,
    pub refresh_credentials: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ListVolumeOptions {
    #[serde(flatten)]
    pub global_options: GlobalOptions,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CleanVolumeOptions {
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
        let mut cmd_args = vec!["setup".to_string()];

        // Add global options
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
        let mut cmd_args = vec!["worktree".to_string(), "create".to_string()];

        // Add global options
        self.add_global_options(&mut cmd_args, &args.global_options);

        // Add task_id
        cmd_args.push(args.task_id);

        let output = self
            .execute_claude_task_command(&cmd_args)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    #[tool(description = "List git worktrees")]
    async fn list_worktree(
        &self,
        Parameters(args): Parameters<ListWorktreeOptions>,
    ) -> Result<CallToolResult, McpError> {
        let mut cmd_args = vec!["worktree".to_string(), "list".to_string()];

        // Add global options
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
        let mut cmd_args = vec!["worktree".to_string(), "remove".to_string()];

        // Add global options
        self.add_global_options(&mut cmd_args, &args.global_options);

        // Add task_id
        cmd_args.push(args.task_id);

        let output = self
            .execute_claude_task_command(&cmd_args)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    #[tool(description = "Initialize Docker volumes")]
    async fn init_volume(
        &self,
        Parameters(args): Parameters<InitVolumeOptions>,
    ) -> Result<CallToolResult, McpError> {
        let mut cmd_args = vec!["volume".to_string(), "init".to_string()];

        // Add global options
        self.add_global_options(&mut cmd_args, &args.global_options);

        // Add refresh credentials option
        if let Some(true) = args.refresh_credentials {
            cmd_args.push("--refresh-credentials".to_string());
        }

        let output = self
            .execute_claude_task_command(&cmd_args)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    #[tool(description = "List Docker volumes")]
    async fn list_volume(
        &self,
        Parameters(args): Parameters<ListVolumeOptions>,
    ) -> Result<CallToolResult, McpError> {
        let mut cmd_args = vec!["volume".to_string(), "list".to_string()];

        // Add global options
        self.add_global_options(&mut cmd_args, &args.global_options);

        let output = self
            .execute_claude_task_command(&cmd_args)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    #[tool(description = "Clean Docker volumes")]
    async fn clean_volume(
        &self,
        Parameters(args): Parameters<CleanVolumeOptions>,
    ) -> Result<CallToolResult, McpError> {
        let mut cmd_args = vec!["volume".to_string(), "clean".to_string()];

        // Add global options
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
        let mut cmd_args = vec!["run".to_string()];

        // Validate approval tool permission format if not empty
        if let Err(e) = ApprovalToolPermission::parse(&args.approval_tool_permission) {
            return Err(McpError::invalid_params(
                format!(
                    "Invalid approval tool permission format: {}\n\nExpected format: mcp__<server_name>__<tool_name>\nExample: mcp__approval_server__approve_command", 
                    e
                ),
                None,
            ));
        }

        // Add global options
        self.add_global_options(&mut cmd_args, &args.global_options);

        // Add run-specific options
        if let Some(task_id) = &args.task_id {
            cmd_args.extend(["--task-id".to_string(), task_id.clone()]);
        }
        if let Some(true) = args.build {
            cmd_args.push("--build".to_string());
        }
        if let Some(workspace_dir) = &args.workspace_dir {
            if let Some(dir) = workspace_dir {
                cmd_args.extend(["--workspace-dir".to_string(), dir.clone()]);
            } else {
                cmd_args.push("--workspace-dir".to_string());
            }
        }
        cmd_args.extend([
            "--approval-tool-permission".to_string(),
            args.approval_tool_permission.clone(),
        ]);
        if let Some(true) = args.debug {
            cmd_args.push("--debug".to_string());
        }
        if let Some(mcp_config) = &args.mcp_config {
            cmd_args.extend(["--mcp-config".to_string(), mcp_config.clone()]);
        }

        // Add the prompt last
        cmd_args.push(args.prompt.clone());

        let output = self
            .execute_claude_task_command(&cmd_args)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    #[tool(description = "Clean up all claude-task git worktrees and docker volumes")]
    async fn clean(
        &self,
        Parameters(args): Parameters<CleanOptions>,
    ) -> Result<CallToolResult, McpError> {
        let mut cmd_args = vec!["clean".to_string()];

        // Add global options
        self.add_global_options(&mut cmd_args, &args.global_options);

        // Always add yes flag, we are deferring to the permission tool to approve or reject
        cmd_args.push("--yes".to_string());

        let output = self
            .execute_claude_task_command(&cmd_args)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

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

#[allow(dead_code)]
#[tokio::main]
async fn main() -> Result<()> {
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
