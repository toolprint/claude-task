use anyhow::Result;

use anyhow::Context;
use futures::{StreamExt, TryStreamExt};
use k8s_openapi::api::{
    batch::v1::{Job, JobSpec},
    core::v1::{Container, EnvVar, EnvVarSource, PodSpec, PodTemplateSpec, SecretKeySelector},
};
use kube::{
    api::{Api, PostParams, WatchEvent, WatchParams},
    Client, Config,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tokio::time::{timeout, Duration};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobConfig {
    pub name: String,
    pub namespace: String,
    pub git_repo: String,
    pub git_branch: Option<String>,
    pub secret_name: String,
    pub secret_key: String,
    pub claude_prompt: String,
    pub claude_permission_tool: Option<String>,
    pub claude_mcp_config: Option<String>,
    pub claude_debug: bool,
    pub claude_skip_permissions: bool,
    pub image: Option<String>,
    pub image_pull_secret: Option<String>,
    pub async_mode: bool,
    pub timeout_seconds: Option<u64>,
    pub oauth_token: Option<String>,
}

#[derive(Debug)]
pub enum JobResult {
    Sync {
        stdout: String,
        stderr: String,
        exit_code: Option<i32>,
    },
    Async {
        job_name: String,
        namespace: String,
    },
}

pub struct KubernetesJobRunner {
    client: Client,
}

impl KubernetesJobRunner {
    /// Create a new KubernetesJobRunner using the default kubeconfig
    pub async fn new() -> Result<Self> {
        let config = Config::infer().await.context(
            "Failed to infer kubernetes config. Please ensure you have a valid kubeconfig file.",
        )?;
        let client = Client::try_from(config)
            .context("Failed to create kubernetes client. Please check your kubeconfig and cluster connectivity.")?;

        Ok(Self { client })
    }

    /// Create and run a Kubernetes Job with the specified configuration
    pub async fn run_job(&self, config: JobConfig) -> Result<JobResult> {
        // Check if git credentials secret exists
        println!(
            "🔍 Checking for git credentials secret '{}'...",
            config.secret_name
        );
        use std::io::{self, Write};
        io::stdout().flush().unwrap();

        let has_git_secret = self
            .validate_secret_exists(&config.namespace, &config.secret_name)
            .await
            .is_ok();

        if !has_git_secret {
            println!(
                "⚠️  Warning: Git credentials secret '{}' not found in namespace '{}'",
                config.secret_name, config.namespace
            );
            println!("   The job will only be able to clone public repositories.");
            println!("   To enable private repository access, run:");
            println!("   claude setup kubernetes");
            println!();
        }

        // Use the configured secret name
        let actual_secret_name = config.secret_name.clone();

        let job = self.create_job_manifest(&config, has_git_secret, &actual_secret_name)?;

        // Submit the job to Kubernetes
        println!("📝 Submitting job to Kubernetes...");
        if has_git_secret {
            println!("   Using git credentials from secret: {actual_secret_name}");
        }
        println!(
            "   Using image: {}",
            job.spec
                .as_ref()
                .and_then(|s| s.template.spec.as_ref())
                .and_then(|s| s.containers.first())
                .and_then(|c| c.image.as_ref())
                .unwrap_or(&"unknown".to_string())
        );

        let api: Api<Job> = Api::namespaced(self.client.clone(), &config.namespace);
        let created_job = match api.create(&PostParams::default(), &job).await {
            Ok(job) => job,
            Err(e) => {
                eprintln!("❌ Failed to create Kubernetes job: {e}");
                return Err(anyhow::anyhow!("Failed to create job. Please check:\n1. Your permissions to create jobs in namespace '{}'\n2. The cluster connectivity\n3. Error details: {}", config.namespace, e));
            }
        };

        // Watch for job completion
        let job_name = created_job
            .metadata
            .name
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Job name not found"))?;

        println!("📋 Job created: {job_name}");

        // If async mode, return immediately
        if config.async_mode {
            println!("🚀 Job started in background mode");
            println!();
            println!("📊 Monitor job status:");
            println!("   kubectl get job {} -n {}", job_name, config.namespace);
            println!();
            println!("📜 View logs:");
            println!(
                "   kubectl logs -f job/{} -n {}",
                job_name, config.namespace
            );
            println!();
            println!("🧹 Clean up when done:");
            println!("   kubectl delete job {} -n {}", job_name, config.namespace);

            return Ok(JobResult::Async {
                job_name: job_name.to_string(),
                namespace: config.namespace.clone(),
            });
        }

        println!("⏳ Waiting for pod to start...");

        let result = self
            .wait_for_completion(&config.namespace, job_name, config.timeout_seconds)
            .await?;

        // Get logs from the job's pod
        let logs = self
            .get_job_logs(&config.namespace, job_name)
            .await
            .unwrap_or_else(|e| {
                eprintln!("⚠️  Failed to get final logs: {e}");
                Logs {
                    stdout: String::new(),
                    stderr: String::new(),
                }
            });

        // Clean up the job (optional - you might want to keep it for debugging)
        if config.claude_debug {
            println!("🔍 Debug mode: Job '{job_name}' not cleaned up");
        } else {
            // self.cleanup_job(&config.namespace, job_name).await?;
        }

        Ok(JobResult::Sync {
            stdout: logs.stdout,
            stderr: logs.stderr,
            exit_code: result.exit_code,
        })
    }

    /// Create the Kubernetes Job manifest
    fn create_job_manifest(
        &self,
        config: &JobConfig,
        has_git_secret: bool,
        actual_secret_name: &str,
    ) -> Result<Job> {
        // Use the provided image or default to GHCR
        let image = config
            .image
            .clone()
            .unwrap_or_else(|| "ghcr.io/onegrep/claude-task:latest".to_string());

        let git_branch = config
            .git_branch
            .as_ref()
            .unwrap_or(&"main".to_string())
            .clone();

        // Use the entrypoint script to ensure proper setup
        let command = vec!["/usr/local/bin/claude-entrypoint.sh".to_string()];

        // Build the claude command
        let mut claude_cmd = vec!["claude".to_string()];

        if config.claude_skip_permissions {
            claude_cmd.push("--dangerously-skip-permissions".to_string());
        } else if let Some(ref permission_tool) = config.claude_permission_tool {
            claude_cmd.push("--permission-prompt-tool".to_string());
            claude_cmd.push(permission_tool.clone());
        }

        if config.claude_debug {
            claude_cmd.push("--debug".to_string());
        }

        if let Some(ref mcp_config) = config.claude_mcp_config {
            claude_cmd.push("--mcp-config".to_string());
            claude_cmd.push(format!("/workspace/{mcp_config}"));
        }

        claude_cmd.push("-p".to_string());
        claude_cmd.push(config.claude_prompt.clone());

        let _claude_command = claude_cmd.join(" ");

        let args = vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            format!(
                r#"
                set -e
                
                # Parse repository URL to extract owner/repo
                REPO_URL="{}"
                
                # Debug: Check environment
                if [ -n "$GIT_TOKEN" ]; then
                    echo "Debug: GIT_TOKEN is set"
                else
                    echo "Debug: GIT_TOKEN is not set"
                fi
                
                # Check if we have git credentials
                if [ -n "$GIT_TOKEN" ]; then
                    echo "Setting up git credentials..."
                    
                    # Extract the repository path from the URL using sed
                    # Handle both https://github.com/owner/repo.git and https://github.com/owner/repo formats
                    if echo "$REPO_URL" | grep -q "github\.com"; then
                        # Extract owner and repo using sed
                        OWNER=$(echo "$REPO_URL" | sed -n 's/.*github\.com[:/]\([^/]*\)\/.*/\1/p')
                        # Extract repo name and remove .git suffix if present
                        REPO=$(echo "$REPO_URL" | sed -n 's/.*github\.com[:/][^/]*\/\([^/]*\)$/\1/p' | sed 's/\.git$//')
                        
                        if [ -n "$OWNER" ] && [ -n "$REPO" ]; then
                            echo "Debug: Parsed OWNER=${{OWNER}}, REPO=${{REPO}}"
                            # Clone using token in URL
                            CLONE_URL="https://${{GIT_TOKEN}}@github.com/${{OWNER}}/${{REPO}}.git"
                            echo "Cloning private repository: github.com/${{OWNER}}/${{REPO}}"
                            echo "Debug: Using authenticated URL"
                        else
                            echo "Warning: Could not parse GitHub repository URL: $REPO_URL"
                            CLONE_URL="$REPO_URL"
                        fi
                    else
                        echo "Warning: Not a GitHub URL, using as-is"
                        CLONE_URL="$REPO_URL"
                    fi
                else
                    echo "No git credentials found, attempting to clone public repository..."
                    CLONE_URL="$REPO_URL"
                fi
                
                echo "Cloning repository..."
                if ! git clone "$CLONE_URL" /workspace; then
                    echo "Failed to clone repository. This may be because:"
                    echo "1. The repository is private and no git credentials were provided"
                    echo "2. The repository URL is incorrect"
                    echo "3. Network connectivity issues"
                    echo "4. Invalid or expired GitHub token"
                    exit 1
                fi
                
                cd /workspace
                
                # Configure git for future operations
                git config --global user.email "claude-task@example.com"
                git config --global user.name "Claude Task"
                
                echo "Creating new branch..."
                git checkout -b {}
                
                echo "Repository cloned successfully to /workspace"
                echo "New branch created: {}"
                echo ""
                
                # Run Claude with the provided prompt
                echo "Running Claude with prompt..."
                echo ""
                
                # Build Claude command
                CLAUDE_CMD="claude"
                
                # Add permission/skip permissions flags
                {}
                
                # Add debug flag if requested
                {}
                
                # Add MCP config if provided
                {}
                
                # Add the prompt
                CLAUDE_CMD="$CLAUDE_CMD -p \"{}\""
                
                echo "Executing: $CLAUDE_CMD"
                echo ""
                
                # Execute Claude
                eval $CLAUDE_CMD
                
                # Capture exit code
                CLAUDE_EXIT=$?
                
                if [ $CLAUDE_EXIT -eq 0 ]; then
                    echo ""
                    echo "✅ Claude task completed successfully"
                else
                    echo ""
                    echo "❌ Claude task failed with exit code: $CLAUDE_EXIT"
                fi
                
                exit $CLAUDE_EXIT
                "#,
                config.git_repo,
                git_branch,
                git_branch,
                // Permission flags
                if config.claude_skip_permissions {
                    r#"CLAUDE_CMD="$CLAUDE_CMD --dangerously-skip-permissions""#.to_string()
                } else if let Some(ref tool) = config.claude_permission_tool {
                    format!(r#"CLAUDE_CMD="$CLAUDE_CMD --permission-prompt-tool \"{tool}\"""#)
                } else {
                    "".to_string()
                },
                // Debug flag
                if config.claude_debug {
                    r#"CLAUDE_CMD="$CLAUDE_CMD --debug""#.to_string()
                } else {
                    "".to_string()
                },
                // MCP config
                if let Some(ref mcp_config) = config.claude_mcp_config {
                    format!(r#"CLAUDE_CMD="$CLAUDE_CMD --mcp-config /workspace/{mcp_config}""#)
                } else {
                    "".to_string()
                },
                config.claude_prompt
            ),
        ];

        // Environment variables that reference the secret
        let mut env_vars = Vec::new();

        // Add Claude configuration directory
        env_vars.push(EnvVar {
            name: "CLAUDE_CONFIG_DIR".to_string(),
            value: Some("/home/node/.claude".to_string()),
            value_from: None,
        });

        // Add debug mode if requested
        if config.claude_debug {
            env_vars.push(EnvVar {
                name: "DEBUG_MODE".to_string(),
                value: Some("true".to_string()),
                value_from: None,
            });
        }

        // Add OAuth token if provided
        if let Some(ref token) = config.oauth_token {
            env_vars.push(EnvVar {
                name: "CLAUDE_CODE_OAUTH_TOKEN".to_string(),
                value: Some(token.clone()),
                value_from: None,
            });
        }

        if has_git_secret {
            env_vars.push(EnvVar {
                name: "GIT_TOKEN".to_string(),
                value: None,
                value_from: Some(EnvVarSource {
                    secret_key_ref: Some(SecretKeySelector {
                        name: Some(actual_secret_name.to_string()),
                        key: config.secret_key.clone(),
                        optional: Some(true), // Make it optional so job can still run
                    }),
                    ..Default::default()
                }),
            });
        }

        // Add volume mounts for Claude credentials
        // Mount the entire secret as /home/base directory structure
        let volume_mounts = vec![k8s_openapi::api::core::v1::VolumeMount {
            name: "claude-home".to_string(),
            mount_path: "/home/base".to_string(),
            read_only: Some(true),
            ..Default::default()
        }];

        let container = Container {
            name: "job-runner".to_string(),
            image: Some(image),
            command: Some(command),
            args: Some(args),
            env: if env_vars.is_empty() {
                None
            } else {
                Some(env_vars)
            },
            working_dir: Some("/workspace".to_string()),
            volume_mounts: Some(volume_mounts),
            ..Default::default()
        };

        // Add image pull secret if configured
        let image_pull_secrets = if let Some(ref secret_name) = config.image_pull_secret {
            Some(vec![k8s_openapi::api::core::v1::LocalObjectReference {
                name: Some(secret_name.clone()),
            }])
        } else {
            None
        };

        // Define volumes
        // Add single volume that recreates the /home/base directory structure
        let volumes = vec![k8s_openapi::api::core::v1::Volume {
            name: "claude-home".to_string(),
            secret: Some(k8s_openapi::api::core::v1::SecretVolumeSource {
                secret_name: Some("claude-credentials".to_string()),
                optional: Some(true), // Make it optional so job can still run without it
                items: Some(vec![
                    k8s_openapi::api::core::v1::KeyToPath {
                        key: "credentials".to_string(),
                        path: ".claude/.credentials.json".to_string(),
                        ..Default::default()
                    },
                    k8s_openapi::api::core::v1::KeyToPath {
                        key: "claude-memory".to_string(),
                        path: ".claude/CLAUDE.md".to_string(),
                        ..Default::default()
                    },
                    k8s_openapi::api::core::v1::KeyToPath {
                        key: "claude-config".to_string(),
                        path: ".claude.json".to_string(),
                        ..Default::default()
                    },
                ]),
                ..Default::default()
            }),
            ..Default::default()
        }];

        let pod_spec = PodSpec {
            containers: vec![container],
            restart_policy: Some("Never".to_string()),
            image_pull_secrets,
            volumes: Some(volumes),
            ..Default::default()
        };

        let pod_template = PodTemplateSpec {
            metadata: Some(Default::default()),
            spec: Some(pod_spec),
        };

        let job_spec = JobSpec {
            template: pod_template,
            backoff_limit: Some(0),                // Don't retry on failure
            ttl_seconds_after_finished: Some(300), // Clean up after 5 minutes
            ..Default::default()
        };

        let mut labels = BTreeMap::new();
        labels.insert("app".to_string(), "job-runner".to_string());
        labels.insert("job-name".to_string(), config.name.clone());

        Ok(Job {
            metadata: k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta {
                name: Some(config.name.clone()),
                namespace: Some(config.namespace.clone()),
                labels: Some(labels),
                ..Default::default()
            },
            spec: Some(job_spec),
            ..Default::default()
        })
    }

    /// Wait for the job to complete
    async fn wait_for_completion(
        &self,
        namespace: &str,
        job_name: &str,
        timeout_seconds: Option<u64>,
    ) -> Result<JobStatus> {
        let api: Api<Job> = Api::namespaced(self.client.clone(), namespace);
        let wp = WatchParams::default()
            .fields(&format!("metadata.name={job_name}"))
            .timeout(30);

        let mut stream = api.watch(&wp, "0").await?.boxed();
        let timeout_duration = Duration::from_secs(timeout_seconds.unwrap_or(300));

        let result = timeout(timeout_duration, async {
            while let Some(event) = stream.try_next().await? {
                match event {
                    WatchEvent::Modified(job) => {
                        if let Some(status) = &job.status {
                            if status.succeeded.unwrap_or(0) > 0 {
                                return Ok(JobStatus {
                                    completed: true,
                                    exit_code: Some(0),
                                });
                            }
                            if status.failed.unwrap_or(0) > 0 {
                                return Ok(JobStatus {
                                    completed: true,
                                    exit_code: Some(1),
                                });
                            }
                        }
                    }
                    WatchEvent::Error(e) => {
                        return Err(anyhow::anyhow!("Watch error: {:?}", e));
                    }
                    _ => {}
                }
            }
            Err(anyhow::anyhow!("Job watch stream ended unexpectedly"))
        })
        .await;

        match result {
            Ok(Ok(status)) => Ok(status),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(anyhow::anyhow!("Job execution timed out")),
        }
    }

    /// Get logs from the job's pod
    async fn get_job_logs(&self, namespace: &str, job_name: &str) -> Result<Logs> {
        use k8s_openapi::api::core::v1::Pod;

        let pod_api: Api<Pod> = Api::namespaced(self.client.clone(), namespace);

        // Find the pod created by the job
        let pods = pod_api
            .list(&Default::default())
            .await
            .context("Failed to list pods")?;

        let job_pod = pods
            .items
            .into_iter()
            .find(|pod| {
                pod.metadata
                    .labels
                    .as_ref()
                    .and_then(|labels| labels.get("job-name"))
                    .map(|name| name == job_name)
                    .unwrap_or(false)
            })
            .ok_or_else(|| anyhow::anyhow!("Pod for job {} not found", job_name))?;

        let pod_name = job_pod
            .metadata
            .name
            .ok_or_else(|| anyhow::anyhow!("Pod name not found"))?;

        // Get logs
        let logs = pod_api
            .logs(&pod_name, &Default::default())
            .await
            .context("Failed to get pod logs")?;

        Ok(Logs {
            stdout: logs,
            stderr: String::new(), // In this simple example, we don't separate stderr
        })
    }

    /// Clean up the job and its pods
    #[allow(dead_code)]
    async fn cleanup_job(&self, namespace: &str, job_name: &str) -> Result<()> {
        let api: Api<Job> = Api::namespaced(self.client.clone(), namespace);
        api.delete(job_name, &Default::default())
            .await
            .context("Failed to delete job")?;
        Ok(())
    }

    /// Validate that a secret exists in the namespace
    async fn validate_secret_exists(&self, namespace: &str, secret_name: &str) -> Result<()> {
        use k8s_openapi::api::core::v1::Secret;

        let api: Api<Secret> = Api::namespaced(self.client.clone(), namespace);
        api.get(secret_name)
            .await
            .map(|_| ())
            .map_err(|e| anyhow::anyhow!("Secret validation failed: {}", e))
    }

    /// Create a git credentials secret
    pub async fn create_git_secret(
        &self,
        namespace: &str,
        secret_name: &str,
        key: &str,
        token: &str,
    ) -> Result<()> {
        use k8s_openapi::api::core::v1::Secret;
        use kube::api::PostParams;
        use std::collections::BTreeMap;

        // Check if secret already exists
        if self
            .validate_secret_exists(namespace, secret_name)
            .await
            .is_ok()
        {
            println!("   ✓ Secret '{secret_name}' already exists");
            return Ok(());
        }

        let mut data = BTreeMap::new();
        data.insert(
            key.to_string(),
            k8s_openapi::ByteString(token.as_bytes().to_vec()),
        );

        let secret = Secret {
            metadata: k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta {
                name: Some(secret_name.to_string()),
                namespace: Some(namespace.to_string()),
                labels: Some({
                    let mut labels = BTreeMap::new();
                    labels.insert("app".to_string(), "claude-task".to_string());
                    labels.insert("temporary".to_string(), "true".to_string());
                    labels
                }),
                ..Default::default()
            },
            data: Some(data),
            ..Default::default()
        };

        let api: Api<Secret> = Api::namespaced(self.client.clone(), namespace);
        api.create(&PostParams::default(), &secret)
            .await
            .map(|_| ())
            .map_err(|e| anyhow::anyhow!("Failed to create secret: {}", e))
    }

    /// Delete a secret
    #[allow(dead_code)]
    async fn delete_secret(&self, namespace: &str, secret_name: &str) -> Result<()> {
        use k8s_openapi::api::core::v1::Secret;
        use kube::api::DeleteParams;

        let api: Api<Secret> = Api::namespaced(self.client.clone(), namespace);
        api.delete(secret_name, &DeleteParams::default())
            .await
            .map(|_| ())
            .map_err(|e| anyhow::anyhow!("Failed to delete secret: {}", e))
    }

    /// Create a namespace if it doesn't exist
    pub async fn create_namespace(&self, namespace: &str) -> Result<()> {
        use k8s_openapi::api::core::v1::Namespace;
        use kube::api::{Api as NamespaceApi, PostParams};

        // Check if namespace already exists
        let api: NamespaceApi<Namespace> = NamespaceApi::all(self.client.clone());
        match api.get(namespace).await {
            Ok(_) => {
                println!("   ✓ Namespace '{namespace}' already exists");
                return Ok(());
            }
            Err(kube::Error::Api(e)) if e.code == 404 => {
                // Namespace doesn't exist, create it
            }
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Failed to check namespace existence: {}",
                    e
                ));
            }
        }

        // Create the namespace
        let ns = Namespace {
            metadata: k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta {
                name: Some(namespace.to_string()),
                labels: Some({
                    let mut labels = BTreeMap::new();
                    labels.insert("app".to_string(), "claude-task".to_string());
                    labels
                }),
                ..Default::default()
            },
            ..Default::default()
        };

        api.create(&PostParams::default(), &ns)
            .await
            .map(|_| {
                println!("   ✓ Namespace '{namespace}' created successfully");
            })
            .map_err(|e| anyhow::anyhow!("Failed to create namespace: {}", e))
    }

    /// Create a docker registry secret for pulling images
    pub async fn create_docker_registry_secret(
        &self,
        namespace: &str,
        secret_name: &str,
        server: &str,
        username: &str,
        password: &str,
    ) -> Result<()> {
        use base64::{engine::general_purpose::STANDARD, Engine as _};
        use k8s_openapi::api::core::v1::Secret;
        use kube::api::PostParams;
        use serde_json::json;
        use std::collections::BTreeMap;

        // Check if secret already exists
        if self
            .validate_secret_exists(namespace, secret_name)
            .await
            .is_ok()
        {
            println!("   ✓ Secret '{secret_name}' already exists");
            return Ok(());
        }

        // Create docker config JSON
        let docker_config = json!({
            "auths": {
                server: {
                    "username": username,
                    "password": password,
                    "auth": STANDARD.encode(format!("{username}:{password}"))
                }
            }
        });

        let docker_config_str = serde_json::to_string(&docker_config)?;

        let mut data = BTreeMap::new();
        data.insert(
            ".dockerconfigjson".to_string(),
            k8s_openapi::ByteString(docker_config_str.into_bytes()),
        );

        let secret = Secret {
            metadata: k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta {
                name: Some(secret_name.to_string()),
                namespace: Some(namespace.to_string()),
                ..Default::default()
            },
            data: Some(data),
            type_: Some("kubernetes.io/dockerconfigjson".to_string()),
            ..Default::default()
        };

        let api: Api<Secret> = Api::namespaced(self.client.clone(), namespace);
        api.create(&PostParams::default(), &secret)
            .await
            .map(|_| {
                println!("   ✓ Created docker registry secret '{secret_name}'");
            })
            .map_err(|e| anyhow::anyhow!("Failed to create docker registry secret: {}", e))
    }

    /// Create a secret containing Claude credentials and configuration
    pub async fn create_claude_credentials_secret(
        &self,
        namespace: &str,
        secret_name: &str,
        home_volume_path: &std::path::Path,
    ) -> Result<()> {
        use k8s_openapi::api::core::v1::Secret;
        use kube::api::PostParams;
        use std::collections::BTreeMap;

        // Check if secret already exists
        if self
            .validate_secret_exists(namespace, secret_name)
            .await
            .is_ok()
        {
            println!("   ✓ Secret '{secret_name}' already exists");
            return Ok(());
        }

        let mut data = BTreeMap::new();

        // Read files from the home volume
        let files_to_include = vec![
            (".claude/.credentials.json", "credentials"),
            (".claude.json", "claude-config"),
            (".claude/CLAUDE.md", "claude-memory"),
        ];

        for (file_path, key) in files_to_include {
            let full_path = home_volume_path.join(file_path);
            if full_path.exists() {
                match std::fs::read(&full_path) {
                    Ok(content) => {
                        data.insert(key.to_string(), k8s_openapi::ByteString(content));
                        println!("   ✓ Added {file_path} to secret");
                    }
                    Err(e) => {
                        println!("   ⚠️  Failed to read {file_path}: {e}");
                    }
                }
            } else {
                println!("   ⚠️  File not found: {file_path}");
            }
        }

        if data.is_empty() {
            return Err(anyhow::anyhow!(
                "No Claude configuration files found. Please run 'ct setup docker' first."
            ));
        }

        let secret = Secret {
            metadata: k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta {
                name: Some(secret_name.to_string()),
                namespace: Some(namespace.to_string()),
                labels: Some({
                    let mut labels = BTreeMap::new();
                    labels.insert("app".to_string(), "claude-task".to_string());
                    labels
                }),
                ..Default::default()
            },
            data: Some(data),
            ..Default::default()
        };

        let api: Api<Secret> = Api::namespaced(self.client.clone(), namespace);
        api.create(&PostParams::default(), &secret)
            .await
            .map(|_| {
                println!("   ✓ Created Claude credentials secret '{secret_name}'");
            })
            .map_err(|e| anyhow::anyhow!("Failed to create Claude credentials secret: {}", e))
    }
}

#[derive(Debug)]
struct JobStatus {
    #[allow(dead_code)]
    completed: bool,
    exit_code: Option<i32>,
}

#[derive(Debug)]
struct Logs {
    stdout: String,
    stderr: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_job_creation() {
        let _config = JobConfig {
            name: "test-job".to_string(),
            namespace: "default".to_string(),
            git_repo: "https://github.com/example/repo.git".to_string(),
            git_branch: Some("claude-task/test".to_string()),
            secret_name: "git-secret".to_string(),
            secret_key: "token".to_string(),
            claude_prompt: "Create a simple hello world script".to_string(),
            claude_permission_tool: None,
            claude_mcp_config: None,
            claude_debug: false,
            claude_skip_permissions: true,
            image: None,
            image_pull_secret: None,
            async_mode: false,
            timeout_seconds: Some(300),
            oauth_token: None,
        };

        // This test would require a running Kubernetes cluster
        // let runner = KubernetesJobRunner::new().await.unwrap();
        // let result = runner.run_job(config).await.unwrap();
        // println!("Job output: {:?}", result);
    }
}

pub async fn run_kubernetes_job(config: JobConfig) -> Result<JobResult> {
    println!("🔧 Creating Kubernetes runner...");
    use std::io::{self, Write};
    io::stdout().flush().unwrap();

    // Create the job runner
    let runner = match KubernetesJobRunner::new().await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("❌ Failed to create Kubernetes client: {e}");
            return Err(e);
        }
    };

    println!("🚀 Starting Kubernetes Claude task...");
    println!("   Job name: {}", config.name);
    println!("   Repository: {}", config.git_repo);
    println!(
        "   Branch: {}",
        config.git_branch.as_ref().unwrap_or(&"main".to_string())
    );
    println!("   Namespace: {}", config.namespace);
    io::stdout().flush().unwrap();

    // Run the job
    println!("\n📋 Creating and running Kubernetes job...");
    io::stdout().flush().unwrap();
    let result = runner.run_job(config).await?;

    // Handle the result based on sync/async mode
    match &result {
        JobResult::Sync {
            stdout,
            stderr,
            exit_code,
        } => {
            println!("\n✅ Job completed");
            if !stdout.is_empty() {
                println!("\n=== JOB OUTPUT ===");
                println!("{stdout}");
            }

            if !stderr.is_empty() {
                eprintln!("\n=== STDERR ===");
                eprintln!("{stderr}");
            }

            if let Some(code) = exit_code {
                println!("\n=== EXIT CODE: {code} ===");
            }
        }
        JobResult::Async {
            job_name,
            namespace,
        } => {
            // Async output is already handled in run_job method
            let _ = (job_name, namespace); // Suppress unused warnings
        }
    }

    Ok(result)
}
