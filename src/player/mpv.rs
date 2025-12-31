use super::traits::{PlayOptions, Player};
use anyhow::{Context, Result};
use std::process::Command;

pub struct MpvPlayer;

impl Player for MpvPlayer {
    fn play(&self, options: PlayOptions) -> Result<()> {
        let mut cmd = Command::new("mpv");

        cmd.arg("--force-window=yes").arg("--keep-open=yes");

        // Handle HTTP Headers
        // MPV format: --http-header-fields="Header1: Value1,Header2: Value2"
        if let Some(headers) = options.headers {
            let header_string = headers
                .iter()
                .map(|(k, v)| format!("{}: {}", k, v))
                .collect::<Vec<String>>()
                .join(",");

            if !header_string.is_empty() {
                cmd.arg(format!("--http-header-fields={}", header_string));
            }
        }

        if let Some(title) = options.title {
            cmd.arg(format!("--title={}", title));
        }

        if let Some(start) = options.start_time {
            cmd.arg(format!("--start={}", start));
        }

        cmd.arg(&options.url);

        println!("▶️  Starting MPV...");

        let status = cmd
            .status()
            .context("Failed to execute 'mpv'. Is it installed and in your PATH?")?;

        if !status.success() {
            anyhow::bail!("MPV exited with non-zero status code");
        }

        Ok(())
    }
}
