use crate::api;
use anyhow::{Context, Result};
use crossterm::style::Stylize;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

const ANILIST_AUTH_URL: &str =
    "https://anilist.co/api/v2/oauth/authorize?client_id=33837&response_type=token";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub general: GeneralConfig,
    pub stream: StreamConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GeneralConfig {
    pub provider: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StreamConfig {
    pub player: String,
    pub quality: String,          // "1080", "720", "480"
    pub translation_type: String, // "sub", "dub"
    pub episode_complete_at: u8,  // Percentage (0-100)
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AuthConfig {
    pub anilist_token: Option<String>,
    pub username: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig {
                provider: "allanime".to_string(),
            },
            stream: StreamConfig {
                player: "mpv".to_string(),
                quality: "1080".to_string(),
                translation_type: "sub".to_string(),
                episode_complete_at: 85,
            },
        }
    }
}

pub struct ConfigManager {
    #[allow(dead_code)]
    pub config_path: PathBuf,
    #[allow(dead_code)]
    auth_path: PathBuf,
    #[allow(dead_code)]
    pub config: Config,
    #[allow(dead_code)]
    pub auth: AuthConfig,
}

impl ConfigManager {
    pub async fn init_interactive() -> Result<Self> {
        let proj_dirs = ProjectDirs::from("com", "sleepy-foundry", "ani-l")
            .context("Could not determine config directory")?;

        let config_dir = proj_dirs.config_dir();
        fs::create_dir_all(config_dir)?;

        let config_path = config_dir.join("config.toml");
        let auth_path = config_dir.join("auth.toml");

        if !config_path.exists() {
            Self::run_setup_wizard(&config_path).await?;
        }

        let config = if config_path.exists() {
            let content = fs::read_to_string(&config_path)?;
            toml::from_str(&content).unwrap_or_default()
        } else {
            Config::default()
        };

        let auth = if auth_path.exists() {
            let content = fs::read_to_string(&auth_path)?;
            toml::from_str(&content).unwrap_or(AuthConfig {
                anilist_token: None,
                username: None,
            })
        } else {
            AuthConfig {
                anilist_token: None,
                username: None,
            }
        };

        let manager = Self {
            config_path,
            auth_path,
            config,
            auth,
        };

        Ok(manager)
    }

    #[allow(dead_code)]
    pub fn new() -> Result<Self> {
        let proj_dirs = ProjectDirs::from("com", "sleepy-foundry", "ani-l")
            .context("Could not determine config directory")?;
        let config_dir = proj_dirs.config_dir();
        fs::create_dir_all(config_dir)?;
        let config_path = config_dir.join("config.toml");
        let auth_path = config_dir.join("auth.toml");

        let config = if config_path.exists() {
            let content = fs::read_to_string(&config_path)?;
            toml::from_str(&content).unwrap_or_default()
        } else {
            let default_config = Config::default();
            let toml_str = toml::to_string_pretty(&default_config)?;
            fs::write(&config_path, toml_str)?;
            default_config
        };

        let auth = if auth_path.exists() {
            let content = fs::read_to_string(&auth_path)?;
            toml::from_str(&content).unwrap_or(AuthConfig {
                anilist_token: None,
                username: None,
            })
        } else {
            AuthConfig {
                anilist_token: None,
                username: None,
            }
        };

        Ok(Self {
            config_path,
            auth_path,
            config,
            auth,
        })
    }

    async fn run_setup_wizard(config_path: &Path) -> Result<()> {
        println!("\n👋 Welcome to ani-l! It looks like this is your first run.");
        println!("📝 Creating configuration file at {:?}", config_path);

        let mut config = Config::default();

        if prompt_bool("Would you like to customize the settings?") {
            // let provider = prompt("Default Provider [allanime]: ");
            // if !provider.is_empty() {
            //     config.general.provider = provider;
            // }

            // let player = prompt("Default Player [mpv]: ");
            // if !player.is_empty() {
            //     config.stream.player = player;
            // }

            let quality = prompt("Default Quality (1080/720/480) [1080]: ");
            if !quality.is_empty() {
                config.stream.quality = quality;
            }

            let trans = prompt("Translation Type (sub/dub) [sub]: ");
            if !trans.is_empty() {
                config.stream.translation_type = trans;
            }
        } else {
            println!("👍 Using default settings.");
        }

        let toml_str = toml::to_string_pretty(&config)?;
        fs::write(config_path, toml_str)?;
        println!("✅ Configuration saved.");

        if prompt_bool("Would you like to authenticate with AniList now?") {
            let proj_dirs = ProjectDirs::from("com", "sleepy-foundry", "ani-l")
                .context("Could not determine config directory")?;

            let config_dir = proj_dirs.config_dir();
            let auth_path = config_dir.join("auth.toml");

            let mut temp_manager = Self {
                config_path: config_path.to_path_buf(),
                auth_path,
                config: config.clone(),
                auth: AuthConfig {
                    anilist_token: None,
                    username: None,
                },
            };

            temp_manager.authenticate_interactive().await?;
        }

        println!("🎉 Setup complete! You can now use ani-l.");
        println!("--------------------------------------------------\n");
        tokio::time::sleep(Duration::from_millis(1500)).await;
        Ok(())
    }

    pub async fn authenticate_interactive(&mut self) -> Result<()> {
        println!("🌍 Opening browser for authentication...");
        println!("🔗 If it doesn't open, visit: {}", ANILIST_AUTH_URL);
        println!("Some terminals won't let you paste the whole token.");
        println!("You can try ");
        println!("{}", "ani-l auth <token>".yellow().bold());

        open_url(ANILIST_AUTH_URL);

        let token = prompt("🔑 Paste your token here: ");
        if token.is_empty() {
            println!("❌ No token provided.");
            return Ok(());
        }

        self.verify_and_save_token(&token).await
    }

    pub async fn verify_and_save_token(&mut self, token: &str) -> Result<()> {
        println!("🔄 Verifying token...");
        match api::authenticate_user(token).await {
            Ok(user) => {
                println!("✅ Successfully logged in as: {}", user.name);
                self.auth.anilist_token = Some(token.to_string());
                self.auth.username = Some(user.name);
                self.save_auth()?;
            }
            Err(e) => {
                eprintln!("❌ Authentication failed: {}", e);
            }
        }
        Ok(())
    }

    pub fn save_auth(&self) -> Result<()> {
        let toml_str = toml::to_string_pretty(&self.auth)?;
        fs::write(&self.auth_path, toml_str)?;
        Ok(())
    }
}

fn prompt(message: &str) -> String {
    print!("{}", message);
    io::stdout().flush().unwrap();
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    input.trim().to_string()
}

fn prompt_bool(message: &str) -> bool {
    loop {
        let input = prompt(&format!("{} (y/n): ", message)).to_lowercase();
        if input == "y" || input == "yes" {
            return true;
        } else if input == "n" || input == "no" {
            return false;
        }
        println!("Please answer 'y' or 'n'.");
    }
}

fn open_url(url: &str) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd")
        .arg("/C")
        .arg("start")
        .arg(url)
        .spawn();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_values() {
        let config = Config::default();

        assert_eq!(config.general.provider, "allanime");
        assert_eq!(config.stream.player, "mpv");
        assert_eq!(config.stream.quality, "1080");
        assert_eq!(config.stream.translation_type, "sub");
        assert_eq!(config.stream.episode_complete_at, 85);
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let toml_str = toml::to_string(&config).expect("Failed to serialize");

        assert!(toml_str.contains("provider = \"allanime\""));
        assert!(toml_str.contains("quality = \"1080\""));
    }
}
