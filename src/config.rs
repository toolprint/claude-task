use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

// Include the generated constants
include!("generated_constants.rs");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, clap::ValueEnum)]
#[serde(rename_all = "camelCase")]
pub enum ExecutionEnvironment {
    Docker,
    Kubernetes,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KubeConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    pub image: String, // e.g., "ghcr.io/{org}/claude-task:latest"
    #[serde(default = "default_git_secret_name")]
    pub git_secret_name: String,
    #[serde(default = "default_git_secret_key")]
    pub git_secret_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_pull_secret: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub namespace_confirmed: bool,
}

fn default_git_secret_name() -> String {
    "git-credentials".to_string()
}

fn default_git_secret_key() -> String {
    "token".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub version: String,
    pub paths: PathConfig,
    pub docker: DockerConfig,
    pub claude_user_config: ClaudeUserConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claude_credentials: Option<ClaudeCredentials>,
    pub worktree: WorktreeConfig,
    pub global_option_defaults: GlobalOptionDefaults,
    #[serde(rename = "taskRunner")]
    pub task_runner: ExecutionEnvironment,
    pub kube_config: Option<KubeConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathConfig {
    pub worktree_base_dir: String,
    pub task_base_home_dir: String,
    pub branch_prefix: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DockerConfig {
    pub image_name: String,
    pub volume_prefix: String,
    pub volumes: DockerVolumes,
    pub container_name_prefix: String,
    pub default_web_view_proxy_port: Option<u16>,
    pub default_ht_mcp_port: Option<u16>,
    pub environment_variables: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DockerVolumes {
    pub home: String,
    pub npm_cache: String,
    pub node_cache: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeUserConfig {
    pub config_path: String,
    pub user_memory_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeCredentials {
    pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeConfig {
    pub default_open_command: Option<String>,
    pub auto_clean_on_remove: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalOptionDefaults {
    pub debug: bool,
    pub open_editor_after_create: bool,
    pub build_image_before_run: bool,
    pub require_ht_mcp: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            paths: PathConfig {
                worktree_base_dir: "~/.claude-task/worktrees".to_string(),
                task_base_home_dir: "~/.claude-task/home".to_string(),
                branch_prefix: "claude-task/".to_string(),
            },
            docker: DockerConfig {
                image_name: DEFAULT_DOCKER_IMAGE.to_string(),
                volume_prefix: "claude-task-".to_string(),
                volumes: DockerVolumes {
                    home: "claude-task-home".to_string(),
                    npm_cache: "claude-task-npm-cache".to_string(),
                    node_cache: "claude-task-node-cache".to_string(),
                },
                container_name_prefix: "claude-task-".to_string(),
                default_web_view_proxy_port: None,
                default_ht_mcp_port: None,
                environment_variables: {
                    let mut env = HashMap::new();
                    env.insert(
                        "CLAUDE_CONFIG_DIR".to_string(),
                        "/home/node/.claude".to_string(),
                    );
                    env
                },
            },
            claude_user_config: ClaudeUserConfig {
                config_path: "~/.claude.json".to_string(),
                user_memory_path: "~/.claude/CLAUDE.md".to_string(),
            },
            claude_credentials: None,
            worktree: WorktreeConfig {
                default_open_command: None,
                auto_clean_on_remove: false,
            },
            global_option_defaults: GlobalOptionDefaults {
                debug: false,
                open_editor_after_create: false,
                build_image_before_run: false,
                require_ht_mcp: false,
            },
            task_runner: ExecutionEnvironment::Docker,
            kube_config: Some(KubeConfig {
                context: None,
                namespace: None,
                image: DEFAULT_DOCKER_IMAGE.to_string(),
                git_secret_name: default_git_secret_name(),
                git_secret_key: default_git_secret_key(),
                image_pull_secret: Some("ghcr-pull-secret".to_string()),
                namespace_confirmed: false,
            }),
        }
    }
}

impl Config {
    pub fn default_config_path() -> PathBuf {
        dirs::home_dir()
            .expect("Could not determine home directory")
            .join(".claude-task")
            .join("config.json")
    }

    /// Generate a unique namespace suffix based on machine metadata
    pub fn generate_namespace_suffix() -> String {
        let mut hasher = Sha256::new();

        // Add hostname
        if let Ok(hostname) = hostname::get() {
            hasher.update(hostname.to_string_lossy().as_bytes());
        }

        // Add MAC addresses
        #[cfg(target_os = "macos")]
        {
            if let Ok(output) = Command::new("ifconfig").output() {
                hasher.update(&output.stdout);
            }
        }

        #[cfg(target_os = "linux")]
        {
            if let Ok(output) = Command::new("ip").args(&["link", "show"]).output() {
                hasher.update(&output.stdout);
            }
        }

        // Add home directory path for additional uniqueness
        if let Some(home) = dirs::home_dir() {
            hasher.update(home.to_string_lossy().as_bytes());
        }

        // Get the hash and convert to hex
        let result = hasher.finalize();
        let hex = format!("{result:x}");

        // Take first 6 characters for a short deterministic suffix
        hex.chars().take(6).collect()
    }

    /// Get the current kubectl context
    pub fn get_current_kube_context() -> Option<String> {
        Command::new("kubectl")
            .args(["config", "current-context"])
            .output()
            .ok()
            .and_then(|output| {
                if output.status.success() {
                    String::from_utf8(output.stdout)
                        .ok()
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                } else {
                    None
                }
            })
    }

    pub fn load(path: Option<&PathBuf>) -> Result<Self> {
        let (config_path, is_custom_path) = match path {
            Some(p) => (p.clone(), true),
            None => (Self::default_config_path(), false),
        };

        if !config_path.exists() {
            if is_custom_path {
                // Error if custom path doesn't exist
                anyhow::bail!(
                    "Config file not found at specified path: {}",
                    config_path.display()
                );
            } else {
                // Create default config at default location
                let default_config = Self::default();
                default_config.save(&config_path).with_context(|| {
                    format!(
                        "Failed to create default config file at: {}",
                        config_path.display()
                    )
                })?;
                println!(
                    "📝 Created default config file at: {}",
                    config_path.display()
                );
                return Ok(default_config);
            }
        }

        let contents = std::fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read config file: {}", config_path.display()))?;

        let config: Self = serde_json::from_str(&contents)
            .with_context(|| format!("Failed to parse config file: {}", config_path.display()))?;

        config
            .validate()
            .with_context(|| format!("Invalid config file: {}", config_path.display()))?;

        Ok(config)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create config directory: {}", parent.display())
            })?;
        }

        let contents = serde_json::to_string_pretty(self).context("Failed to serialize config")?;

        std::fs::write(path, contents)
            .with_context(|| format!("Failed to write config file: {}", path.display()))?;

        // Set file permissions to 600 (read/write for owner only)
        #[cfg(unix)]
        {
            use std::fs;
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(path)?.permissions();
            perms.set_mode(0o600);
            fs::set_permissions(path, perms).with_context(|| {
                format!(
                    "Failed to set permissions on config file: {}",
                    path.display()
                )
            })?;
        }

        Ok(())
    }

    /// Check if ht-mcp binary is available in the system
    pub fn check_ht_mcp_availability() -> bool {
        // Try common installation paths and system PATH
        let ht_mcp_paths = [
            "ht-mcp",                                // System PATH
            "/usr/local/bin/ht-mcp",                 // Common installation location
            "/usr/bin/ht-mcp",                       // System binary location
            "/opt/homebrew/bin/ht-mcp",              // Homebrew on Apple Silicon
            "/home/linuxbrew/.linuxbrew/bin/ht-mcp", // Linuxbrew
        ];

        for path in &ht_mcp_paths {
            if Command::new(path).arg("--version").output().is_ok() {
                return true;
            }
        }

        false
    }

    pub fn validate(&self) -> Result<()> {
        // Validate version
        if self.version.is_empty() {
            anyhow::bail!("Config version cannot be empty");
        }

        // Validate paths
        if self.paths.worktree_base_dir.is_empty() {
            anyhow::bail!("worktreeBaseDir cannot be empty");
        }
        if self.paths.task_base_home_dir.is_empty() {
            anyhow::bail!("taskBaseHomeDir cannot be empty");
        }
        if self.paths.branch_prefix.is_empty() {
            anyhow::bail!("branchPrefix cannot be empty");
        }

        // Validate Docker settings
        if self.docker.image_name.is_empty() {
            anyhow::bail!("Docker imageName cannot be empty");
        }
        if self.docker.volume_prefix.is_empty() {
            anyhow::bail!("Docker volumePrefix cannot be empty");
        }
        if self.docker.container_name_prefix.is_empty() {
            anyhow::bail!("Docker containerNamePrefix cannot be empty");
        }

        // Validate Docker volumes
        if self.docker.volumes.home.is_empty() {
            anyhow::bail!("Docker volumes.home cannot be empty");
        }
        if self.docker.volumes.npm_cache.is_empty() {
            anyhow::bail!("Docker volumes.npmCache cannot be empty");
        }
        if self.docker.volumes.node_cache.is_empty() {
            anyhow::bail!("Docker volumes.nodeCache cannot be empty");
        }

        // Validate port if specified
        if let Some(port) = self.docker.default_web_view_proxy_port {
            if port == 0 {
                anyhow::bail!("defaultWebViewProxyPort must be greater than 0 or null");
            }
        }

        // Validate Claude user config
        if self.claude_user_config.config_path.is_empty() {
            anyhow::bail!("claudeUserConfig.configPath cannot be empty");
        }
        if self.claude_user_config.user_memory_path.is_empty() {
            anyhow::bail!("claudeUserConfig.userMemoryPath cannot be empty");
        }

        // Validate ht-mcp availability if required
        if self.global_option_defaults.require_ht_mcp && !Self::check_ht_mcp_availability() {
            anyhow::bail!(
                "ht-mcp binary is required but not found. Please install ht-mcp or set 'requireHtMcp' to false in config."
            );
        }

        // Validate Kubernetes config if task runner is Kubernetes
        if let ExecutionEnvironment::Kubernetes = self.task_runner {
            if let Some(kube_config) = &self.kube_config {
                // Context and namespace can be None (will be detected/generated)
                if kube_config.image.is_empty() {
                    anyhow::bail!("Kubernetes image cannot be empty");
                }
            } else {
                anyhow::bail!("Kubernetes task runner requires a kube_config");
            }
        }

        Ok(())
    }

    pub fn expand_tilde(path: &str) -> PathBuf {
        if let Some(stripped) = path.strip_prefix("~/") {
            dirs::home_dir()
                .expect("Could not determine home directory")
                .join(stripped)
        } else {
            PathBuf::from(path)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_default_config_is_valid() {
        let config = Config::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_save_and_load_config() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");

        let config = Config::default();
        config.save(&config_path).unwrap();

        let loaded = Config::load(Some(&config_path)).unwrap();
        assert_eq!(loaded.version, config.version);
        assert_eq!(
            loaded.paths.worktree_base_dir,
            config.paths.worktree_base_dir
        );
    }

    #[test]
    fn test_expand_tilde() {
        let home = dirs::home_dir().unwrap();
        let expanded = Config::expand_tilde("~/test");
        assert_eq!(expanded, home.join("test"));

        let no_tilde = Config::expand_tilde("/absolute/path");
        assert_eq!(no_tilde, PathBuf::from("/absolute/path"));
    }

    #[test]
    fn test_invalid_config_validation() {
        let mut config = Config::default();
        config.paths.worktree_base_dir = String::new();
        assert!(config.validate().is_err());

        let mut config = Config::default();
        config.docker.image_name = String::new();
        assert!(config.validate().is_err());
    }
}
