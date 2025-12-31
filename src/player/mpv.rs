// src/player/mpv.rs
use super::traits::{EpisodeAction, EpisodeNavigator, PlayOptions, Player};
use anyhow::{Context, Result};
use serde_json::json;
use std::process::Command;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::time::sleep;

pub struct MpvPlayer;

impl Player for MpvPlayer {
    async fn play(&self, options: PlayOptions, navigator: Option<EpisodeNavigator>) -> Result<f64> {
        // 1. Setup Socket Path
        let socket_id = rand::random::<u32>();
        let socket_path = format!("/tmp/ani-l-mpv-{}.sock", socket_id);

        // 2. Spawn MPV
        let mut cmd = Command::new("mpv");
        cmd.arg("--force-window=yes")
            .arg("--keep-open=yes")
            .arg(format!("--input-ipc-server={}", socket_path))
            .arg("--term-osd-bar")
            .arg("--term-status-msg=Status: ${time-pos} / ${duration} (${percent-pos}%)");

        if let Some(headers) = &options.headers {
            let h_str = headers
                .iter()
                .map(|(k, v)| format!("{}: {}", k, v))
                .collect::<Vec<_>>()
                .join(",");
            if !h_str.is_empty() {
                cmd.arg(format!("--http-header-fields={}", h_str));
            }
        }
        if let Some(title) = &options.title {
            cmd.arg(format!("--title={}", title));
        }
        if let Some(start) = &options.start_time {
            cmd.arg(format!("--start={}", start));
        }

        if let Some(subtitles) = &options.subtitles {
            for sub in subtitles {
                cmd.arg(format!("--sub-file={}", sub));
            }
        }

        cmd.arg(&options.url);

        println!("▶️  Starting MPV (IPC)...");
        let mut child = cmd.spawn().context("Failed to spawn MPV")?;

        // 3. Connect to IPC (Retry loop)
        let mut stream = None;
        for _ in 0..20 {
            if let Ok(s) = UnixStream::connect(&socket_path).await {
                stream = Some(s);
                break;
            }
            sleep(Duration::from_millis(100)).await;
        }

        #[allow(unused_mut)]
        let mut max_percentage = 0.0;

        if let Some(stream) = stream {
            let (reader, mut writer) = stream.into_split();
            let buf_reader = BufReader::new(reader);
            let mut lines = buf_reader.lines();

            // --- KEY BINDINGS ---
            // FIX: Combine script-message and arg into ONE string, just like 'viu' does.
            // Also use lowercase 'shift+n' which is standard MPV syntax.
            let bindings = [
                ("shift+n", "script-message next-episode"),
                ("N", "script-message next-episode"),
                ("shift+p", "script-message previous-episode"),
                ("P", "script-message previous-episode"),
            ];

            for (key, cmd_str) in bindings {
                // Correct format: ["keybind", "key", "command string"]
                let cmd = json!({ "command": ["keybind", key, cmd_str] });
                let _ = writer.write_all(cmd.to_string().as_bytes()).await;
                let _ = writer.write_all(b"\n").await;
            }
            let _ = writer.flush().await;
            // --------------------

            let observe_cmd = json!({ "command": ["observe_property", 1, "percent-pos"] });
            let _ = writer.write_all(observe_cmd.to_string().as_bytes()).await;
            let _ = writer.write_all(b"\n").await;
            let _ = writer.flush().await;

            loop {
                tokio::select! {
                    _ = sleep(Duration::from_millis(100)) => {
                        if let Ok(Some(_)) = child.try_wait() {
                            break;
                        }
                    }
                    line = lines.next_line() => {
                        match line {
                            Ok(Some(msg)) => {
                                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&msg)
                                    && let Some(event) = val.get("event").and_then(|e| e.as_str()) {
                                        if event == "client-message" {
                                            if let Some(args) = val.get("args").and_then(|a| a.as_array())
                                                && !args.is_empty() {
                                                    // Parse action
                                                    let action = match args[0].as_str() {
                                                        Some("next-episode") => Some(EpisodeAction::Next),
                                                        Some("previous-episode") => Some(EpisodeAction::Previous),
                                                        _ => None
                                                    };

                                                    if let (Some(act), Some(nav)) = (action, &navigator) {
                                                        let label = match act {
                                                            EpisodeAction::Next => "Next",
                                                            EpisodeAction::Previous => "Previous",
                                                        };

                                                        // Show loading OSD
                                                        let _ = writer.write_all(json!({ "command": ["show-text", format!("Fetching {} Episode...", label), "5000"] }).to_string().as_bytes()).await;
                                                        let _ = writer.write_all(b"\n").await;
                                                        let _ = writer.flush().await;

                                                        match nav(act).await {
                                                            Ok(Some(new_opts)) => {
                                                                let load_cmd = json!({ "command": ["loadfile", new_opts.url] });
                                                                let _ = writer.write_all(load_cmd.to_string().as_bytes()).await;
                                                                let _ = writer.write_all(b"\n").await;

                                                                if let Some(t) = new_opts.title {
                                                                    let title_cmd = json!({ "command": ["set_property", "title", t] });
                                                                    let _ = writer.write_all(title_cmd.to_string().as_bytes()).await;
                                                                    let _ = writer.write_all(b"\n").await;
                                                                }

                                                                // Reset percentage for the new episode
                                                                max_percentage = 0.0;
                                                                let _ = writer.flush().await;
                                                            }
                                                            Ok(None) => {
                                                                let _ = writer.write_all(json!({ "command": ["show-text", format!("No {} episode found", label.to_lowercase())] }).to_string().as_bytes()).await;
                                                                let _ = writer.write_all(b"\n").await;
                                                                let _ = writer.flush().await;
                                                            }
                                                            Err(e) => {
                                                                let _ = writer.write_all(json!({ "command": ["show-text", format!("Error: {}", e)] }).to_string().as_bytes()).await;
                                                                let _ = writer.write_all(b"\n").await;
                                                                let _ = writer.flush().await;
                                                            }
                                                        }
                                                    }
                                                }
                                        } else if event == "property-change"
                                            && let Some(name) = val.get("name").and_then(|n| n.as_str())
                                                && name == "percent-pos"
                                                    && let Some(p) = val.get("data").and_then(|d| d.as_f64())
                                                        && p > max_percentage { max_percentage = p; }
                                    }
                            }
                            Ok(None) => break,
                            Err(_) => break,
                        }
                    }
                }
            }
        } else {
            let _ = child.wait();
        }

        let _ = child.wait();

        if std::path::Path::new(&socket_path).exists() {
            let _ = std::fs::remove_file(&socket_path);
        }

        Ok(max_percentage)
    }
}
