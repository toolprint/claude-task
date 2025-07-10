use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::sleep;

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncLock {
    pub task_id: String,
    pub pid: u32,
    pub timestamp: u64,
    pub hostname: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LastSync {
    pub synced_at: u64,
    pub credential_hash: String,
    pub synced_by: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LastValidated {
    pub validated_at: u64,
    pub validated_by: String,
    pub credential_hash: String,
}

pub struct CredentialSyncManager {
    metadata_dir: String,
    task_id: String,
}

impl CredentialSyncManager {
    pub fn new(task_base_home_dir: &str, task_id: &str) -> Result<Self> {
        let metadata_dir = format!("{task_base_home_dir}/.credential_metadata");
        fs::create_dir_all(&metadata_dir)
            .with_context(|| format!("Failed to create metadata directory: {metadata_dir}"))?;

        Ok(Self {
            metadata_dir,
            task_id: task_id.to_string(),
        })
    }

    pub async fn sync_credentials_if_needed<F, Fut>(
        &self,
        sync_callback: F,
        debug: bool,
    ) -> Result<bool>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<String>>,
    {
        // Step 1: Check if sync is needed
        if let Ok(Some(validated)) = self.read_last_validated() {
            let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
            let elapsed = now.saturating_sub(validated.validated_at);

            if elapsed < 300 {
                // Less than 5 minutes
                if debug {
                    println!("✓ Credentials validated {elapsed} seconds ago, skipping sync");
                }
                return Ok(false);
            }
        }

        // Step 2: Attempt to acquire lock
        let lock_path = format!("{}/sync_lock", self.metadata_dir);
        let mut attempts = 0;

        loop {
            match self.acquire_sync_lock(&lock_path) {
                Ok(_) => {
                    if debug {
                        println!("🔒 Acquired sync lock");
                    }
                    break;
                }
                Err(_) if attempts < 6 => {
                    // Check if lock is stale (older than 60s)
                    if self.is_lock_stale(&lock_path, 60)? {
                        if debug {
                            println!("🔓 Removing stale lock");
                        }
                        let _ = fs::remove_file(&lock_path);
                        continue;
                    }

                    // Wait and retry
                    attempts += 1;
                    if debug {
                        println!(
                            "⏳ Sync lock held by another process, waiting... (attempt {attempts}/6)"
                        );
                    }
                    sleep(Duration::from_secs(10)).await;

                    // Re-check if sync still needed after wait
                    if let Ok(Some(validated)) = self.read_last_validated() {
                        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
                        let elapsed = now.saturating_sub(validated.validated_at);

                        if elapsed < 300 {
                            if debug {
                                println!("✓ Another process synced credentials, skipping");
                            }
                            return Ok(false);
                        }
                    }
                }
                Err(e) => {
                    return Err(anyhow::anyhow!(
                        "Failed to acquire sync lock after {} attempts: {}",
                        attempts,
                        e
                    ));
                }
            }
        }

        // Step 3: Perform sync with lock held
        let sync_result = async {
            if debug {
                println!("🔄 Performing credential sync...");
            }

            // Call the sync callback to get credentials
            let credentials = sync_callback().await?;

            // Calculate hash
            let hash = self.calculate_hash(&credentials);

            // Update last sync metadata
            let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
            let last_sync = LastSync {
                synced_at: now,
                credential_hash: hash.clone(),
                synced_by: self.task_id.clone(),
            };
            self.write_last_sync(&last_sync)?;

            // Also update last validated since we just synced
            let last_validated = LastValidated {
                validated_at: now,
                validated_by: self.task_id.clone(),
                credential_hash: hash,
            };
            self.write_last_validated(&last_validated)?;

            Ok::<bool, anyhow::Error>(true)
        }
        .await;

        // Step 4: Release lock
        let _ = fs::remove_file(&lock_path);

        sync_result
    }

    pub fn update_validation_timestamp(&self) -> Result<()> {
        // Read current credential hash from last sync
        let last_sync = self.read_last_sync()?;
        let credential_hash = last_sync
            .map(|s| s.credential_hash)
            .unwrap_or_else(|| "unknown".to_string());

        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let last_validated = LastValidated {
            validated_at: now,
            validated_by: self.task_id.clone(),
            credential_hash,
        };

        self.write_last_validated(&last_validated)?;
        Ok(())
    }

    fn acquire_sync_lock(&self, lock_path: &str) -> Result<()> {
        let lock_data = SyncLock {
            task_id: self.task_id.clone(),
            pid: std::process::id(),
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            hostname: hostname::get()
                .unwrap_or_else(|_| std::ffi::OsString::from("unknown"))
                .to_string_lossy()
                .to_string(),
        };

        let json = serde_json::to_string_pretty(&lock_data)?;

        // Use OpenOptions with create_new for atomic lock creation
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(lock_path)
            .and_then(|mut file| file.write_all(json.as_bytes()))
            .map_err(|e| anyhow::anyhow!("Failed to create lock file: {}", e))
    }

    fn is_lock_stale(&self, lock_path: &str, max_age_secs: u64) -> Result<bool> {
        let content = match fs::read_to_string(lock_path) {
            Ok(c) => c,
            Err(_) => return Ok(false),
        };

        let lock: SyncLock = match serde_json::from_str(&content) {
            Ok(l) => l,
            Err(_) => return Ok(true), // Corrupted lock file is considered stale
        };

        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let age = now.saturating_sub(lock.timestamp);

        if age > max_age_secs {
            // Check if the process is still running
            if !self.is_process_running(lock.pid) {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn is_process_running(&self, pid: u32) -> bool {
        // Platform-specific process checking
        #[cfg(target_os = "macos")]
        {
            use std::process::Command;
            Command::new("kill")
                .args(["-0", &pid.to_string()])
                .output()
                .map(|output| output.status.success())
                .unwrap_or(false)
        }

        #[cfg(not(target_os = "macos"))]
        {
            // For other platforms, check if /proc/{pid} exists
            Path::new(&format!("/proc/{}", pid)).exists()
        }
    }

    fn calculate_hash(&self, content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        format!("sha256:{:x}", hasher.finalize())
    }

    fn read_last_sync(&self) -> Result<Option<LastSync>> {
        let path = format!("{}/last_sync", self.metadata_dir);
        match fs::read_to_string(&path) {
            Ok(content) => {
                let last_sync: LastSync =
                    serde_json::from_str(&content).context("Failed to parse last_sync metadata")?;
                Ok(Some(last_sync))
            }
            Err(_) => Ok(None),
        }
    }

    fn write_last_sync(&self, last_sync: &LastSync) -> Result<()> {
        let path = format!("{}/last_sync", self.metadata_dir);
        let json = serde_json::to_string_pretty(last_sync)?;
        fs::write(&path, json)
            .with_context(|| format!("Failed to write last_sync metadata to {path}"))
    }

    fn read_last_validated(&self) -> Result<Option<LastValidated>> {
        let path = format!("{}/last_validated", self.metadata_dir);
        match fs::read_to_string(&path) {
            Ok(content) => {
                let last_validated: LastValidated = serde_json::from_str(&content)
                    .context("Failed to parse last_validated metadata")?;
                Ok(Some(last_validated))
            }
            Err(_) => Ok(None),
        }
    }

    fn write_last_validated(&self, last_validated: &LastValidated) -> Result<()> {
        let path = format!("{}/last_validated", self.metadata_dir);
        let json = serde_json::to_string_pretty(last_validated)?;
        fs::write(&path, json)
            .with_context(|| format!("Failed to write last_validated metadata to {path}"))
    }

    pub fn is_credential_error(error_message: &str) -> bool {
        // Common credential error patterns
        error_message.contains("unauthorized")
            || error_message.contains("401")
            || error_message.contains("authentication failed")
            || error_message.contains("invalid credentials")
            || error_message.contains("token expired")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_credential_sync_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let _manager =
            CredentialSyncManager::new(temp_dir.path().to_str().unwrap(), "test-task-123").unwrap();

        let metadata_dir = temp_dir.path().join(".credential_metadata");
        assert!(metadata_dir.exists());
    }

    #[tokio::test]
    async fn test_sync_needed_when_no_validation() {
        let temp_dir = TempDir::new().unwrap();
        let manager =
            CredentialSyncManager::new(temp_dir.path().to_str().unwrap(), "test-task-123").unwrap();

        let synced = manager
            .sync_credentials_if_needed(|| async { Ok("test-credentials".to_string()) }, false)
            .await
            .unwrap();

        assert!(synced);
    }

    #[tokio::test]
    async fn test_sync_skipped_when_recently_validated() {
        let temp_dir = TempDir::new().unwrap();
        let manager =
            CredentialSyncManager::new(temp_dir.path().to_str().unwrap(), "test-task-123").unwrap();

        // First sync
        let synced = manager
            .sync_credentials_if_needed(|| async { Ok("test-credentials".to_string()) }, false)
            .await
            .unwrap();
        assert!(synced);

        // Second sync immediately after should be skipped
        let synced = manager
            .sync_credentials_if_needed(|| async { Ok("test-credentials".to_string()) }, false)
            .await
            .unwrap();
        assert!(!synced);
    }

    #[test]
    fn test_hash_calculation() {
        let temp_dir = TempDir::new().unwrap();
        let manager =
            CredentialSyncManager::new(temp_dir.path().to_str().unwrap(), "test-task-123").unwrap();

        let hash1 = manager.calculate_hash("test-content");
        let hash2 = manager.calculate_hash("test-content");
        let hash3 = manager.calculate_hash("different-content");

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
        assert!(hash1.starts_with("sha256:"));
    }

    #[test]
    fn test_credential_error_detection() {
        assert!(CredentialSyncManager::is_credential_error(
            "Error: unauthorized access"
        ));
        assert!(CredentialSyncManager::is_credential_error(
            "HTTP 401 Unauthorized"
        ));
        assert!(CredentialSyncManager::is_credential_error(
            "authentication failed"
        ));
        assert!(CredentialSyncManager::is_credential_error(
            "invalid credentials provided"
        ));
        assert!(CredentialSyncManager::is_credential_error("token expired"));

        assert!(!CredentialSyncManager::is_credential_error(
            "file not found"
        ));
        assert!(!CredentialSyncManager::is_credential_error(
            "network timeout"
        ));
    }

    #[test]
    fn test_lock_acquisition() {
        let temp_dir = TempDir::new().unwrap();
        let manager =
            CredentialSyncManager::new(temp_dir.path().to_str().unwrap(), "test-task-123").unwrap();

        let lock_path = format!("{}/sync_lock", manager.metadata_dir);

        // First acquisition should succeed
        assert!(manager.acquire_sync_lock(&lock_path).is_ok());

        // Second acquisition should fail
        assert!(manager.acquire_sync_lock(&lock_path).is_err());

        // Remove lock
        fs::remove_file(&lock_path).unwrap();

        // Now acquisition should succeed again
        assert!(manager.acquire_sync_lock(&lock_path).is_ok());
    }

    #[test]
    fn test_update_validation_timestamp() {
        let temp_dir = TempDir::new().unwrap();
        let manager =
            CredentialSyncManager::new(temp_dir.path().to_str().unwrap(), "test-task-123").unwrap();

        // Update validation timestamp
        manager.update_validation_timestamp().unwrap();

        // Read it back
        let validated = manager.read_last_validated().unwrap();
        assert!(validated.is_some());

        let validated = validated.unwrap();
        assert_eq!(validated.validated_by, "test-task-123");
        assert_eq!(validated.credential_hash, "unknown"); // No sync done yet
    }

    #[tokio::test]
    async fn test_parallel_sync_with_lock() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path().to_str().unwrap().to_string();

        // Create a shared counter to track how many syncs happen
        use std::sync::{Arc, Mutex};
        let sync_count = Arc::new(Mutex::new(0));

        // Spawn multiple tasks that try to sync simultaneously
        let mut handles = vec![];

        for i in 0..3 {
            let path = temp_path.clone();
            let count = sync_count.clone();

            let handle = tokio::spawn(async move {
                let manager = CredentialSyncManager::new(&path, &format!("task-{i}")).unwrap();

                let synced = manager
                    .sync_credentials_if_needed(
                        || {
                            let c = count.clone();
                            async move {
                                let mut c = c.lock().unwrap();
                                *c += 1;
                                Ok(format!("credentials-{i}"))
                            }
                        },
                        false,
                    )
                    .await
                    .unwrap();

                synced
            });

            handles.push(handle);
        }

        // Wait for all tasks to complete
        let results: Vec<bool> = futures_util::future::join_all(handles)
            .await
            .into_iter()
            .map(|r| r.unwrap())
            .collect();

        // Only one task should have performed the sync
        let sync_performed_count = results.iter().filter(|&&synced| synced).count();
        assert_eq!(sync_performed_count, 1);

        // The sync callback should have been called only once
        assert_eq!(*sync_count.lock().unwrap(), 1);
    }

    #[test]
    fn test_stale_lock_detection() {
        let temp_dir = TempDir::new().unwrap();
        let manager =
            CredentialSyncManager::new(temp_dir.path().to_str().unwrap(), "test-task-123").unwrap();

        let lock_path = format!("{}/sync_lock", manager.metadata_dir);

        // Create a lock with old timestamp
        let stale_lock = SyncLock {
            task_id: "old-task".to_string(),
            pid: 99999, // Non-existent PID
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
                - 120, // 2 minutes old
            hostname: "old-host".to_string(),
        };

        let json = serde_json::to_string_pretty(&stale_lock).unwrap();
        fs::write(&lock_path, json).unwrap();

        // Check if lock is stale
        assert!(manager.is_lock_stale(&lock_path, 60).unwrap());
    }

    #[tokio::test]
    async fn test_validation_within_window() {
        let temp_dir = TempDir::new().unwrap();
        let manager =
            CredentialSyncManager::new(temp_dir.path().to_str().unwrap(), "test-task-123").unwrap();

        // First sync
        let synced = manager
            .sync_credentials_if_needed(|| async { Ok("test-credentials".to_string()) }, false)
            .await
            .unwrap();
        assert!(synced);

        // Create another manager instance (simulating another process)
        let manager2 =
            CredentialSyncManager::new(temp_dir.path().to_str().unwrap(), "test-task-456").unwrap();

        // Second sync from different task should be skipped due to recent validation
        let synced2 = manager2
            .sync_credentials_if_needed(|| async { Ok("test-credentials-2".to_string()) }, false)
            .await
            .unwrap();
        assert!(!synced2);
    }

    #[test]
    fn test_metadata_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_str().unwrap();

        // Create manager and write metadata
        {
            let manager = CredentialSyncManager::new(path, "test-task-123").unwrap();

            let last_sync = LastSync {
                synced_at: 12345,
                credential_hash: "test-hash".to_string(),
                synced_by: "test-task".to_string(),
            };
            manager.write_last_sync(&last_sync).unwrap();

            let last_validated = LastValidated {
                validated_at: 12346,
                validated_by: "test-task".to_string(),
                credential_hash: "test-hash".to_string(),
            };
            manager.write_last_validated(&last_validated).unwrap();
        }

        // Create new manager and read metadata
        {
            let manager = CredentialSyncManager::new(path, "test-task-456").unwrap();

            let sync = manager.read_last_sync().unwrap().unwrap();
            assert_eq!(sync.synced_at, 12345);
            assert_eq!(sync.credential_hash, "test-hash");
            assert_eq!(sync.synced_by, "test-task");

            let validated = manager.read_last_validated().unwrap().unwrap();
            assert_eq!(validated.validated_at, 12346);
            assert_eq!(validated.validated_by, "test-task");
            assert_eq!(validated.credential_hash, "test-hash");
        }
    }
}
