use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

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

    #[allow(dead_code)]
    pub fn save_auth(&self) -> Result<()> {
        let toml_str = toml::to_string_pretty(&self.auth)?;
        fs::write(&self.auth_path, toml_str)?;
        Ok(())
    }
}
