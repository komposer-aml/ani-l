use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum WatchStatus {
    CURRENT,
    PLANNING,
    COMPLETED,
    DROPPED,
    PAUSED,
    REPEATING,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RegistryEntry {
    pub id: i32,
    pub title: String,
    pub status: WatchStatus,
    pub progress: i32,
    pub total_episodes: Option<i32>,
    pub score: f32,
    pub last_updated: DateTime<Utc>,
    #[serde(default)]
    pub dirty: bool,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Registry {
    pub entries: HashMap<i32, RegistryEntry>,
}

pub struct RegistryManager {
    #[allow(dead_code)]
    file_path: PathBuf,
    #[allow(dead_code)]
    pub data: Registry,
}

impl RegistryManager {
    pub fn new() -> Result<Self> {
        let proj_dirs = ProjectDirs::from("com", "sleepy-foundry", "ani-l")
            .context("Could not determine config directory")?;
        let file_path = proj_dirs.config_dir().join("registry.json");

        let data = if file_path.exists() {
            let content = fs::read_to_string(&file_path)?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            Registry::default()
        };

        Ok(Self { file_path, data })
    }

    #[allow(dead_code)]
    pub fn save(&self) -> Result<()> {
        let json_str = serde_json::to_string_pretty(&self.data)?;
        fs::write(&self.file_path, json_str)?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn update_entry(&mut self, entry: RegistryEntry) -> Result<()> {
        self.data.entries.insert(entry.id, entry);
        self.save()
    }

    #[allow(dead_code)]
    pub fn get_entry(&self, id: i32) -> Option<&RegistryEntry> {
        self.data.entries.get(&id)
    }
}
