use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use localauthentication_rs::{LAPolicy, LocalAuthentication};
use security_framework::passwords::get_generic_password;
use serde::{Deserialize, Serialize};

#[derive(Parser)]
#[command(name = "claude-task-keychain-helper")]
#[command(about = "macOS keychain helper for claude-task", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Extract credentials from macOS keychain
    Extract {
        /// Service name in keychain
        #[arg(short, long)]
        service: String,

        /// Account name in keychain
        #[arg(short, long)]
        account: String,

        /// Skip biometric authentication
        #[arg(long)]
        no_biometric: bool,
    },

    /// Check if biometric authentication is available
    CheckBiometric,
}

#[derive(Serialize, Deserialize)]
struct CredentialResponse {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    credentials: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Extract {
            service,
            account,
            no_biometric,
        } => extract_credentials(&service, &account, !no_biometric),
        Commands::CheckBiometric => check_biometric_availability(),
    }
}

fn extract_credentials(service: &str, account: &str, use_biometric: bool) -> Result<()> {
    // Request biometric authentication if enabled
    if use_biometric {
        if let Err(e) = request_biometric_authentication() {
            let response = CredentialResponse {
                success: false,
                credentials: None,
                error: Some(format!("Biometric authentication failed: {}", e)),
            };
            println!("{}", serde_json::to_string(&response)?);
            return Ok(());
        }
    }

    // Try to get credentials from macOS keychain using Security framework
    match get_generic_password(service, account) {
        Ok(password_data) => {
            let password =
                String::from_utf8(password_data).context("Password data is not valid UTF-8")?;

            let response = CredentialResponse {
                success: true,
                credentials: Some(password),
                error: None,
            };
            println!("{}", serde_json::to_string(&response)?);
            Ok(())
        }
        Err(e) => {
            // Fall back to keyring crate
            match keyring::Entry::new(service, account) {
                Ok(entry) => match entry.get_password() {
                    Ok(password) => {
                        let response = CredentialResponse {
                            success: true,
                            credentials: Some(password),
                            error: None,
                        };
                        println!("{}", serde_json::to_string(&response)?);
                        Ok(())
                    }
                    Err(e) => {
                        let response = CredentialResponse {
                            success: false,
                            credentials: None,
                            error: Some(format!("Failed to get credentials: {}", e)),
                        };
                        println!("{}", serde_json::to_string(&response)?);
                        Ok(())
                    }
                },
                Err(e) => {
                    let response = CredentialResponse {
                        success: false,
                        credentials: None,
                        error: Some(format!(
                            "Failed to access keychain: {} (Security framework error: {:?})",
                            e, e
                        )),
                    };
                    println!("{}", serde_json::to_string(&response)?);
                    Ok(())
                }
            }
        }
    }
}

fn request_biometric_authentication() -> Result<()> {
    let local_auth = LocalAuthentication::new();

    // Check if biometric authentication is available
    let policy = LAPolicy::DeviceOwnerAuthenticationWithBiometrics;

    if !local_auth.can_evaluate_policy(policy) {
        return Err(anyhow::anyhow!(
            "Biometric authentication not available on this device"
        ));
    }

    eprintln!("🔐 Requesting biometric authentication (Touch ID/Face ID)...");

    // Request biometric authentication
    let success =
        local_auth.evaluate_policy(policy, "Claude Task needs to access your credentials");

    if success {
        eprintln!("✓ Biometric authentication successful");
        Ok(())
    } else {
        Err(anyhow::anyhow!("Biometric authentication failed"))
    }
}

fn check_biometric_availability() -> Result<()> {
    let local_auth = LocalAuthentication::new();
    let policy = LAPolicy::DeviceOwnerAuthenticationWithBiometrics;

    let available = local_auth.can_evaluate_policy(policy);

    let response = serde_json::json!({
        "available": available
    });

    println!("{}", serde_json::to_string(&response)?);
    Ok(())
}
