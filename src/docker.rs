use anyhow::{Context, Result};
use bollard::{
    container::{
        Config, CreateContainerOptions, LogOutput, LogsOptions, RemoveContainerOptions,
        StartContainerOptions, WaitContainerOptions,
    },
    image::BuildImageOptions,
    models::{BuildInfo, HostConfig, Mount, MountTypeEnum, RestartPolicy, RestartPolicyNameEnum},
    volume::{CreateVolumeOptions, ListVolumesOptions},
    Docker,
};
use futures_util::stream::StreamExt;
use std::collections::HashMap;
use std::path::Path;

pub struct DockerManager {
    docker: Docker,
}

#[derive(Debug, Clone)]
pub struct ClaudeTaskConfig {
    pub task_id: String,
    pub workspace_path: String,
    pub timezone: String,
    pub dockerfile_path: String,
    pub context_path: String,
}

impl Default for ClaudeTaskConfig {
    fn default() -> Self {
        Self {
            task_id: "default".to_string(),
            workspace_path: std::env::current_dir()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            timezone: "America/New_York".to_string(),
            dockerfile_path: "Dockerfile".to_string(),
            context_path: "claude-task".to_string(),
        }
    }
}

impl DockerManager {
    pub fn new() -> Result<Self> {
        let docker =
            Docker::connect_with_local_defaults().context("Failed to connect to Docker daemon")?;
        Ok(Self { docker })
    }

    /// Create necessary volumes for Claude task
    pub async fn create_volumes(&self, _config: &ClaudeTaskConfig) -> Result<()> {
        let volumes = vec![
            (
                "claude-task-npm-cache".to_string(),
                "Shared npm cache volume".to_string(),
            ),
            (
                "claude-task-node-cache".to_string(),
                "Shared node cache volume".to_string(),
            ),
        ];

        for (volume_name, description) in volumes {
            let create_options = CreateVolumeOptions {
                name: volume_name.clone(),
                labels: {
                    let mut labels = HashMap::new();
                    labels.insert("description".to_string(), description);
                    labels.insert("project".to_string(), "claude-task".to_string());
                    labels
                },
                ..Default::default()
            };

            match self.docker.create_volume(create_options).await {
                Ok(_) => println!("✓ Volume '{}' created", volume_name),
                Err(e) if e.to_string().contains("already exists") => {
                    println!("✓ Volume '{}' already exists", volume_name)
                }
                Err(e) => {
                    return Err(anyhow::anyhow!(
                        "Failed to create volume '{}': {}",
                        volume_name,
                        e
                    ))
                }
            }
        }

        Ok(())
    }

    /// Check if a Docker image exists
    pub async fn check_image_exists(&self, image_name: &str) -> Result<()> {
        match self.docker.inspect_image(image_name).await {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow::anyhow!("Image '{}' not found: {}", image_name, e)),
        }
    }

    /// Build the Claude task image
    pub async fn build_image(&self, config: &ClaudeTaskConfig) -> Result<()> {
        println!("🔨 Building Claude task image...");

        let dockerfile_path = Path::new(&config.dockerfile_path);
        let context_path = Path::new(&config.context_path);

        if !dockerfile_path.exists() {
            return Err(anyhow::anyhow!(
                "Dockerfile not found at: {}",
                config.dockerfile_path
            ));
        }

        if !context_path.exists() {
            return Err(anyhow::anyhow!(
                "Build context not found at: {}",
                config.context_path
            ));
        }

        // Create build options
        let build_options = BuildImageOptions {
            dockerfile: "Dockerfile".to_string(),
            t: "claude-task:dev".to_string(),
            buildargs: {
                let mut args = HashMap::new();
                args.insert("TZ".to_string(), config.timezone.clone());
                args
            },
            ..Default::default()
        };

        // Create tar archive of build context
        let tar_data = self.create_tar_archive(context_path)?;

        // Build image
        let mut stream = self
            .docker
            .build_image(build_options, None, Some(tar_data.into()));

        while let Some(result) = stream.next().await {
            match result {
                Ok(BuildInfo {
                    stream: Some(output),
                    ..
                }) => {
                    print!("{}", output);
                }
                Ok(BuildInfo {
                    error: Some(error), ..
                }) => {
                    return Err(anyhow::anyhow!("Build error: {}", error));
                }
                Err(e) => return Err(anyhow::anyhow!("Build stream error: {}", e)),
                _ => {}
            }
        }

        println!("✓ Image built successfully");
        Ok(())
    }

    /// Run Claude task container
    pub async fn run_claude_task(
        &self,
        config: &ClaudeTaskConfig,
        prompt: &str,
        permission_prompt_tool: &str,
        debug: bool,
        mcp_config: Option<String>,
        skip_permissions: bool,
    ) -> Result<()> {
        println!("🚀 Starting Claude task container...");

        // Create container configuration
        let container_config = self
            .create_container_config(
                config,
                prompt,
                permission_prompt_tool,
                debug,
                mcp_config,
                skip_permissions,
            )
            .await?;

        let container_name = format!("claude-task-{}", config.task_id);

        // Remove existing container if it exists
        let remove_options = RemoveContainerOptions {
            force: true,
            ..Default::default()
        };
        let _ = self
            .docker
            .remove_container(&container_name, Some(remove_options))
            .await;

        // Create container with auto-remove
        let create_options = CreateContainerOptions {
            name: container_name.clone(),
            ..Default::default()
        };

        let container = self
            .docker
            .create_container(Some(create_options), container_config)
            .await
            .context("Failed to create container")?;

        println!("✓ Container created: {}", container.id);

        // Start container
        self.docker
            .start_container(&container.id, None::<StartContainerOptions<String>>)
            .await
            .context("Failed to start container")?;

        println!("✓ Container started");

        // Stream logs
        self.stream_container_logs(&container.id).await?;

        // Wait for container to finish
        let wait_options = WaitContainerOptions {
            condition: "not-running".to_string(),
        };

        let mut wait_stream = self
            .docker
            .wait_container(&container.id, Some(wait_options));
        if let Some(result) = wait_stream.next().await {
            match result {
                Ok(wait_result) => {
                    if wait_result.status_code != 0 {
                        return Err(anyhow::anyhow!(
                            "Container exited with non-zero status: {}",
                            wait_result.status_code
                        ));
                    }
                }
                Err(e) => return Err(anyhow::anyhow!("Wait error: {}", e)),
            }
        }

        // Container will auto-remove itself due to auto_remove: true

        println!("✅ Claude task completed successfully!");
        Ok(())
    }

    async fn create_container_config(
        &self,
        config: &ClaudeTaskConfig,
        prompt: &str,
        permission_prompt_tool: &str,
        debug: bool,
        mcp_config: Option<String>,
        skip_permissions: bool,
    ) -> Result<Config<String>> {
        // Create mounts for volumes
        let mut mounts = vec![
            Mount {
                target: Some("/home/base".to_string()),
                source: Some("claude-task-home".to_string()),
                typ: Some(MountTypeEnum::VOLUME),
                read_only: Some(true),
                ..Default::default()
            },
            Mount {
                target: Some("/home/node/.npm".to_string()),
                source: Some("claude-task-npm-cache".to_string()),
                typ: Some(MountTypeEnum::VOLUME),
                ..Default::default()
            },
            Mount {
                target: Some("/home/node/.cache".to_string()),
                source: Some("claude-task-node-cache".to_string()),
                typ: Some(MountTypeEnum::VOLUME),
                ..Default::default()
            },
            Mount {
                target: Some("/workspace".to_string()),
                source: Some(config.workspace_path.clone()),
                typ: Some(MountTypeEnum::BIND),
                ..Default::default()
            },
        ];

        // Add tasks.mcp.json mount if the file exists
        let tasks_mcp_path = Path::new(&config.workspace_path).join("tasks.mcp.json");
        if tasks_mcp_path.exists() {
            mounts.push(Mount {
                target: Some("/workspace/.mcp.json".to_string()),
                source: Some(tasks_mcp_path.to_string_lossy().to_string()),
                typ: Some(MountTypeEnum::BIND),
                read_only: Some(true),
                ..Default::default()
            });
        }

        // Add custom MCP config mount if provided
        if let Some(ref mcp_config_path) = mcp_config {
            let source_path = Path::new(mcp_config_path);
            if source_path.exists() {
                mounts.push(Mount {
                    target: Some("/home/node/task.mcp.json".to_string()),
                    source: Some(source_path.to_string_lossy().to_string()),
                    typ: Some(MountTypeEnum::BIND),
                    read_only: Some(true),
                    ..Default::default()
                });
            }
        }

        // Environment variables
        let env_vars = vec![
            format!("TASK_ID={}", config.task_id),
            "NODE_OPTIONS=--max-old-space-size=4096".to_string(),
            "CLAUDE_CONFIG_DIR=/home/node/.claude".to_string(),
            "POWERLEVEL9K_DISABLE_GITSTATUS=true".to_string(),
        ];

        let host_config = HostConfig {
            mounts: Some(mounts),
            restart_policy: Some(RestartPolicy {
                name: Some(RestartPolicyNameEnum::NO),
                ..Default::default()
            }),
            auto_remove: Some(true),
            ..Default::default()
        };

        // Build the claude command
        let mut claude_cmd = vec!["claude".to_string()];

        if skip_permissions {
            claude_cmd.push("--dangerously-skip-permissions".to_string());
        } else {
            claude_cmd.push("--permission-prompt-tool".to_string());
            claude_cmd.push(permission_prompt_tool.to_string());
        }

        if debug {
            claude_cmd.push("--debug".to_string());
        }

        if let Some(ref mcp_config_path) = mcp_config {
            let source_path = Path::new(mcp_config_path);
            if source_path.exists() {
                claude_cmd.push("--mcp-config".to_string());
                claude_cmd.push("/home/node/task.mcp.json".to_string());
            }
        }

        claude_cmd.extend(vec![
            "-p".to_string(),
            format!("\"{}\"", prompt.replace("\"", "\\\"")),
        ]);

        // Create shell command that first copies base directory contents to /home/node, then runs claude
        let claude_cmd_str = claude_cmd.join(" ");
        let full_cmd = format!(
            "cp -r /home/base/. /home/node/ 2>/dev/null || true && {}",
            claude_cmd_str
        );

        if debug {
            println!("🔍 Container command:");
            println!("   Shell: sh -c");
            println!("   Full command: {}", full_cmd);
            println!("   Claude command: {}", claude_cmd_str);
        }

        let cmd = vec!["sh".to_string(), "-c".to_string(), full_cmd];

        let config = Config {
            image: Some("claude-task:dev".to_string()),
            cmd: Some(cmd),
            env: Some(env_vars),
            working_dir: Some("/workspace".to_string()),
            host_config: Some(host_config),
            ..Default::default()
        };

        Ok(config)
    }

    async fn stream_container_logs(&self, container_id: &str) -> Result<()> {
        let logs_options = LogsOptions::<String> {
            follow: true,
            stdout: true,
            stderr: true,
            ..Default::default()
        };

        let mut log_stream = self.docker.logs(container_id, Some(logs_options));

        while let Some(result) = log_stream.next().await {
            match result {
                Ok(LogOutput::StdOut { message }) => {
                    print!("{}", String::from_utf8_lossy(&message));
                }
                Ok(LogOutput::StdErr { message }) => {
                    eprint!("{}", String::from_utf8_lossy(&message));
                }
                Ok(_) => {}
                Err(e) => {
                    eprintln!("Log stream error: {}", e);
                    break;
                }
            }
        }

        Ok(())
    }

    fn create_tar_archive(&self, context_path: &Path) -> Result<Vec<u8>> {
        use std::io::Cursor;
        use tar::Builder;

        let mut archive_data = Vec::new();
        {
            let cursor = Cursor::new(&mut archive_data);
            let mut archive = Builder::new(cursor);

            // Add all files in the context directory
            archive
                .append_dir_all(".", context_path)
                .context("Failed to create tar archive")?;

            archive.finish().context("Failed to finalize tar archive")?;
        }

        Ok(archive_data)
    }

    /// List volumes related to Claude tasks
    pub async fn list_claude_volumes(&self) -> Result<Vec<(String, String)>> {
        let list_options = ListVolumesOptions::<String> {
            filters: {
                let mut filters = HashMap::new();
                filters.insert("label".to_string(), vec!["project=claude-task".to_string()]);
                filters
            },
        };

        let volumes_response = self
            .docker
            .list_volumes(Some(list_options))
            .await
            .context("Failed to list volumes")?;

        let mut volume_info = Vec::new();
        for volume in volumes_response.volumes.unwrap_or_default() {
            let name = volume.name;
            // Get volume size by inspecting it
            let size = self
                .get_volume_size(&name)
                .await
                .unwrap_or_else(|_| "unknown".to_string());
            volume_info.push((name, size));
        }

        Ok(volume_info)
    }

    /// Get the size of a Docker volume
    async fn get_volume_size(&self, volume_name: &str) -> Result<String> {
        use std::process::Command;

        // Use docker run with a temporary container to calculate volume size
        let output = Command::new("docker")
            .args([
                "run",
                "--rm",
                "-v",
                &format!("{}:/vol", volume_name),
                "alpine",
                "du",
                "-sh",
                "/vol",
            ])
            .output()
            .context("Failed to execute docker run command for volume size")?;

        if !output.status.success() {
            return Ok("unknown".to_string());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let size = stdout
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().next())
            .unwrap_or("unknown")
            .to_string();

        Ok(size)
    }

    /// Check if claude-task-home volume exists
    pub async fn check_home_volume_exists(&self) -> Result<bool> {
        match self.docker.inspect_volume("claude-task-home").await {
            Ok(_) => Ok(true),
            Err(e) if e.to_string().contains("no such volume") => Ok(false),
            Err(e) => Err(anyhow::anyhow!("Failed to check volume: {}", e)),
        }
    }
}
