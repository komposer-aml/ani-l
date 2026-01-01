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
    #[serde(default = "default_language")]
    pub language: String,
}

fn default_language() -> String {
    "en".to_string()
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
                language: "en".to_string(),
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
        let language = select_language();

        rust_i18n::set_locale(&language);

        println!("{}", t!("setup.welcome"));
        println!(
            "{}",
            t!("setup.creating_config", path = format!("{:?}", config_path))
        );

        let mut config = Config::default();
        config.general.language = language;

        if prompt_bool(&t!("setup.customize_prompt")) {
            let quality = prompt(&t!("setup.quality_prompt"));
            if !quality.is_empty() {
                config.stream.quality = quality;
            }

            let trans = prompt(&t!("setup.translation_prompt"));
            if !trans.is_empty() {
                config.stream.translation_type = trans;
            }
        } else {
            println!("{}", t!("setup.use_defaults"));
        }

        let toml_str = toml::to_string_pretty(&config)?;
        fs::write(config_path, toml_str)?;
        println!("{}", t!("setup.saved"));

        if prompt_bool(&t!("setup.auth_prompt")) {
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

        println!("{}", t!("setup.complete"));
        println!("--------------------------------------------------\n");
        tokio::time::sleep(Duration::from_millis(1500)).await;
        Ok(())
    }

    pub async fn authenticate_interactive(&mut self) -> Result<()> {
        println!("{}", t!("setup.auth_browser"));
        println!("{}", t!("setup.auth_link", url = ANILIST_AUTH_URL));
        println!("{}", t!("setup.auth_tip_1"));
        println!("{}", t!("setup.auth_tip_2"));
        println!("{}", "ani-l auth <token>".yellow().bold());

        open_url(ANILIST_AUTH_URL);

        let token = prompt(&t!("setup.token_prompt"));
        if token.is_empty() {
            println!("{}", t!("setup.no_token"));
            return Ok(());
        }

        self.verify_and_save_token(&token).await
    }

    pub async fn verify_and_save_token(&mut self, token: &str) -> Result<()> {
        println!("{}", t!("setup.verifying"));
        match api::authenticate_user(token).await {
            Ok(user) => {
                println!("{}", t!("setup.logged_in", name = user.name));
                self.auth.anilist_token = Some(token.to_string());
                self.auth.username = Some(user.name);
                self.save_auth()?;
            }
            Err(e) => {
                eprintln!("{}", t!("setup.auth_failed", error = e.to_string()));
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

fn select_language() -> String {
    println!("\n🌐 Select Language / Seleccione el idioma:");
    println!("1. English (en)");
    println!("2. Español (es)");
    println!("3. Português (pt)");
    println!("4. Français (fr)");
    println!("5. Bahasa Indonesia (id)");
    println!("6. Русский (ru)");

    loop {
        print!("{}", "\n> ".cyan().bold());
        io::stdout().flush().unwrap_or(());

        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(_) => match input.trim() {
                "1" => return "en".to_string(),
                "2" => return "es".to_string(),
                "3" => return "pt".to_string(),
                "4" => return "fr".to_string(),
                "5" => return "id".to_string(),
                "6" => return "ru".to_string(),
                _ => {
                    println!("❌ Invalid selection. Please enter 1-6.");
                }
            },
            Err(_) => return "en".to_string(),
        }
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
        assert_eq!(config.general.language, "en");
        assert_eq!(config.stream.player, "mpv");
        assert_eq!(config.stream.quality, "1080");
        assert_eq!(config.stream.translation_type, "sub");
        assert_eq!(config.stream.episode_complete_at, 85);
    }
}
