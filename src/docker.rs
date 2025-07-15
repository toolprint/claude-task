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

use crate::config::DockerConfig;

pub struct DockerManager {
    docker: Docker,
    config: DockerConfig,
}

#[derive(Debug, Clone)]
pub enum TaskRunResult {
    Sync {
        output: String,
    },
    Async {
        task_id: String,
        container_id: String,
    },
}

#[derive(Debug, Clone)]
pub struct ClaudeTaskConfig {
    pub task_id: String,
    pub workspace_path: String,
    pub timezone: String,
    pub dockerfile_path: String,
    pub context_path: String,
    pub ht_mcp_port: Option<u16>,
    pub web_view_proxy_port: Option<u16>,
}

#[derive(Debug, Clone)]
pub struct RunTaskOptions {
    pub prompt: String,
    pub permission_prompt_tool: String,
    pub debug: bool,
    pub mcp_config: Option<String>,
    pub skip_permissions: bool,
    pub async_mode: bool,
    pub oauth_token: Option<String>,
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
            ht_mcp_port: None,
            web_view_proxy_port: None,
        }
    }
}

impl DockerManager {
    pub fn new(config: DockerConfig) -> Result<Self> {
        let docker =
            Docker::connect_with_local_defaults().context("Failed to connect to Docker daemon")?;
        Ok(Self { docker, config })
    }

    /// Create necessary volumes for Claude task
    pub async fn create_volumes(&self, _config: &ClaudeTaskConfig) -> Result<()> {
        let volumes = vec![
            (
                self.config.volumes.npm_cache.clone(),
                "Shared npm cache volume".to_string(),
            ),
            (
                self.config.volumes.node_cache.clone(),
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
                Ok(_) => println!("✓ Volume '{volume_name}' created"),
                Err(e) if e.to_string().contains("already exists") => {
                    println!("✓ Volume '{volume_name}' already exists")
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
            t: self.config.image_name.clone(),
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
                    print!("{output}");
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
        options: &RunTaskOptions,
    ) -> Result<TaskRunResult> {
        println!("🚀 Starting Claude task container...");

        // Create container configuration
        let container_config = self.create_container_config(config, options).await?;

        let container_name = format!("{}{}", self.config.container_name_prefix, config.task_id);

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

        if options.async_mode {
            // Return immediately for async mode
            println!("📋 Task started in background mode");
            println!("   Task ID: {}", config.task_id);
            println!("   Container ID: {}", container.id);
            println!("   Monitor with: docker logs {container_name}");

            Ok(TaskRunResult::Async {
                task_id: config.task_id.clone(),
                container_id: container.id,
            })
        } else {
            // Show waiting indicator
            println!();
            println!("⏳ Waiting for Claude's response...");

            // Stream logs and parse output for sync mode
            let claude_output = self
                .stream_and_parse_logs(&container.id, options.debug)
                .await?;

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

            println!();
            println!("=============== 💬 CLAUDE'S RESPONSE END 💬 ===============");
            println!();

            Ok(TaskRunResult::Sync {
                output: claude_output,
            })
        }
    }

    async fn create_container_config(
        &self,
        config: &ClaudeTaskConfig,
        options: &RunTaskOptions,
    ) -> Result<Config<String>> {
        // Create mounts for volumes
        let mut mounts = vec![
            Mount {
                target: Some("/home/base".to_string()),
                source: Some(self.config.volumes.home.clone()),
                typ: Some(MountTypeEnum::VOLUME),
                read_only: Some(true),
                ..Default::default()
            },
            Mount {
                target: Some("/home/node/.npm".to_string()),
                source: Some(self.config.volumes.npm_cache.clone()),
                typ: Some(MountTypeEnum::VOLUME),
                ..Default::default()
            },
            Mount {
                target: Some("/home/node/.cache".to_string()),
                source: Some(self.config.volumes.node_cache.clone()),
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
        if let Some(ref mcp_config_path) = options.mcp_config {
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
        let mut env_vars = vec![
            format!("TASK_ID={}", config.task_id),
            "NODE_OPTIONS=--max-old-space-size=4096".to_string(),
            "CLAUDE_CONFIG_DIR=/home/node/.claude".to_string(),
            "POWERLEVEL9K_DISABLE_GITSTATUS=true".to_string(),
            format!(
                "DEBUG_MODE={}",
                if options.debug { "true" } else { "false" }
            ),
        ];

        // Add CCO MCP URL if HT-MCP is enabled
        if config.ht_mcp_port.is_some() {
            env_vars.push("CCO_MCP_URL=http://host.docker.internal:8660/mcp".to_string());
        }

        // Add OAuth token if provided
        if let Some(ref token) = options.oauth_token {
            env_vars.push(format!("CLAUDE_CODE_OAUTH_TOKEN={token}"));
        }

        let mut host_config = HostConfig {
            mounts: Some(mounts),
            restart_policy: Some(RestartPolicy {
                name: Some(RestartPolicyNameEnum::NO),
                ..Default::default()
            }),
            auto_remove: Some(!options.async_mode), // Don't auto-remove in async mode
            ..Default::default()
        };

        // Add port mapping for web view proxy if specified
        if let Some(port) = config.web_view_proxy_port {
            if port > 0 {
                use bollard::models::PortBinding;
                use std::collections::HashMap;

                let mut port_bindings = HashMap::new();

                // Only expose nginx proxy port (container 4618 -> host configured port)
                port_bindings.insert(
                    "4618/tcp".to_string(),
                    Some(vec![PortBinding {
                        host_ip: Some("0.0.0.0".to_string()),
                        host_port: Some(port.to_string()),
                    }]),
                );

                host_config.port_bindings = Some(port_bindings);

                println!("🌐 Web interface will be available at:");
                println!("   Web Proxy: http://localhost:{port} (NGINX proxy with fallback page)");
            }
        }

        // Build the claude command
        let mut claude_cmd = vec!["claude".to_string()];

        if options.skip_permissions {
            claude_cmd.push("--dangerously-skip-permissions".to_string());
        } else {
            claude_cmd.push("--permission-prompt-tool".to_string());
            claude_cmd.push(options.permission_prompt_tool.to_string());
        }

        if options.debug {
            claude_cmd.push("--debug".to_string());
        }

        if let Some(ref mcp_config_path) = options.mcp_config {
            let source_path = Path::new(mcp_config_path);
            if source_path.exists() {
                claude_cmd.push("--mcp-config".to_string());
                claude_cmd.push("/home/node/task.mcp.json".to_string());
            }
        }

        claude_cmd.extend(vec!["-p".to_string(), options.prompt.to_string()]);

        // The entrypoint script will run automatically, we just need to pass the claude command
        let cmd = claude_cmd;

        if options.debug {
            println!("🔍 Container command:");
            println!("   Claude command: {}", cmd.join(" "));
            println!("   (Entrypoint script will run automatically)");
        }

        let mut container_config = Config {
            image: Some(self.config.image_name.clone()),
            cmd: Some(cmd),
            env: Some(env_vars),
            working_dir: Some("/workspace".to_string()),
            host_config: Some(host_config),
            ..Default::default()
        };

        // Add exposed ports if web view proxy is enabled
        if let Some(port) = config.web_view_proxy_port {
            if port > 0 {
                use std::collections::HashMap;
                let mut exposed_ports = HashMap::new();
                exposed_ports.insert("4618/tcp".to_string(), HashMap::new()); // nginx proxy only
                container_config.exposed_ports = Some(exposed_ports);
            }
        }

        Ok(container_config)
    }

    async fn stream_and_parse_logs(&self, container_id: &str, debug: bool) -> Result<String> {
        let logs_options = LogsOptions::<String> {
            follow: true,
            stdout: true,
            stderr: true,
            ..Default::default()
        };

        let mut log_stream = self.docker.logs(container_id, Some(logs_options));
        let mut claude_output = String::new();
        let mut capturing_claude = false;
        let mut response_started = false;

        while let Some(result) = log_stream.next().await {
            match result {
                Ok(LogOutput::StdOut { message }) => {
                    let text = String::from_utf8_lossy(&message);

                    // Print the BEGIN marker when we first receive stdout output
                    if !response_started && capturing_claude {
                        println!();
                        println!("=============== 💬 CLAUDE'S RESPONSE BEGIN 💬 ===============");
                        println!();
                        response_started = true;
                    }

                    // Always stream stdout to the user
                    print!("{text}");
                    // Capture it for return value
                    claude_output.push_str(&text);
                }
                Ok(LogOutput::StdErr { message }) => {
                    let text = String::from_utf8_lossy(&message);

                    // Check for output markers
                    if text.contains("=== CLAUDE_OUTPUT_START ===") {
                        capturing_claude = true;
                        if debug {
                            eprintln!("{text}");
                        }
                    } else if text.contains("=== CLAUDE_OUTPUT_END ===") {
                        capturing_claude = false;
                        if debug {
                            eprintln!("{text}");
                        }
                    } else if debug || !capturing_claude {
                        // Show setup logs in debug mode or when not in Claude output section
                        eprint!("{text}");
                    }
                }
                Ok(_) => {}
                Err(e) => {
                    eprintln!("Log stream error: {e}");
                    break;
                }
            }
        }

        Ok(claude_output)
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
    #[allow(dead_code)]
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
    #[allow(dead_code)]
    async fn get_volume_size(&self, volume_name: &str) -> Result<String> {
        use std::process::Command;

        // Use docker run with a temporary container to calculate volume size
        let output = Command::new("docker")
            .args([
                "run",
                "--rm",
                "-v",
                &format!("{volume_name}:/vol"),
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
        match self.docker.inspect_volume(&self.config.volumes.home).await {
            Ok(_) => Ok(true),
            Err(e) if e.to_string().contains("no such volume") => Ok(false),
            Err(e) => Err(anyhow::anyhow!("Failed to check volume: {}", e)),
        }
    }
}
