use super::traits::{NextEpisodeResolver, PlayOptions, Player};
use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::json;
use std::process::Command;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::time::sleep;

pub struct MpvPlayer;

#[derive(Serialize)]
struct IpcCommand {
    command: Vec<serde_json::Value>,
}

impl Player for MpvPlayer {
    async fn play(
        &self,
        options: PlayOptions,
        next_resolver: Option<NextEpisodeResolver>,
    ) -> Result<f64> {
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

        cmd.arg(&options.url);

        println!("▶️  Starting MPV (IPC)...");
        let mut child = cmd.spawn().context("Failed to spawn MPV")?;

        // 3. Connect to IPC (Retry loop)
        let mut stream = None;
        for _ in 0..20 {
            // Try for 2 seconds
            if let Ok(s) = UnixStream::connect(&socket_path).await {
                stream = Some(s);
                break;
            }
            sleep(Duration::from_millis(100)).await;
        }

        #[allow(unused_mut)]
        let mut max_percentage = 0.0;

        if let Some(stream) = stream {
            // 4. Setup IPC environment
            let (reader, mut writer) = stream.into_split();
            let mut buf_reader = BufReader::new(reader);
            let mut lines = buf_reader.lines();

            // Bind Shift+N to a script message
            let bind_cmd =
                json!({ "command": ["keybind", "Shift+N", "script-message", "next-episode"] });
            let _ = writer.write_all(bind_cmd.to_string().as_bytes()).await;
            let _ = writer.write_all(b"\n").await;

            // Observe properties for watch tracking
            let observe_cmd = json!({ "command": ["observe_property", 1, "percent-pos"] });
            let _ = writer.write_all(observe_cmd.to_string().as_bytes()).await;
            let _ = writer.write_all(b"\n").await;

            // 5. Event Loop
            loop {
                tokio::select! {
                    // Check if MPV closed
                    _ = sleep(Duration::from_millis(100)) => {
                        if let Ok(Some(_)) = child.try_wait() {
                            break;
                        }
                    }
                    // Handle IPC Messages
                    line = lines.next_line() => {
                        match line {
                            Ok(Some(msg)) => {
                                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&msg) {
                                    // Handle Events
                                    if let Some(event) = val.get("event").and_then(|e| e.as_str()) {
                                        if event == "client-message" {
                                            if let Some(args) = val.get("args").and_then(|a| a.as_array()) {
                                                if !args.is_empty() && args[0] == "next-episode" {
                                                     if let Some(resolver) = &next_resolver {
                                                         println!("\n⏭️  Fetching Next Episode...");
                                                         let _ = writer.write_all(json!({ "command": ["show-text", "Fetching Next Episode..."] }).to_string().as_bytes()).await;
                                                         let _ = writer.write_all(b"\n").await;

                                                         match resolver().await {
                                                             Ok(Some(new_opts)) => {
                                                                 // Load new file
                                                                 let load_cmd = json!({ "command": ["loadfile", new_opts.url] });
                                                                 let _ = writer.write_all(load_cmd.to_string().as_bytes()).await;
                                                                 let _ = writer.write_all(b"\n").await;

                                                                 if let Some(t) = new_opts.title {
                                                                     let title_cmd = json!({ "command": ["set_property", "title", t] });
                                                                     let _ = writer.write_all(title_cmd.to_string().as_bytes()).await;
                                                                     let _ = writer.write_all(b"\n").await;
                                                                 }
                                                                 println!("✅ Loaded Next Episode");
                                                             }
                                                             Ok(None) => {
                                                                 println!("❌ No next episode found.");
                                                                 let _ = writer.write_all(json!({ "command": ["show-text", "No next episode found"] }).to_string().as_bytes()).await;
                                                                 let _ = writer.write_all(b"\n").await;
                                                             }
                                                             Err(e) => {
                                                                 eprintln!("❌ Error fetching next: {}", e);
                                                                 let _ = writer.write_all(json!({ "command": ["show-text", format!("Error: {}", e)] }).to_string().as_bytes()).await;
                                                                 let _ = writer.write_all(b"\n").await;
                                                             }
                                                         }
                                                     }
                                                }
                                            }
                                        } else if event == "property-change" {
                                            if let Some(name) = val.get("name").and_then(|n| n.as_str()) {
                                                if name == "percent-pos" {
                                                    if let Some(p) = val.get("data").and_then(|d| d.as_f64()) {
                                                        if p > max_percentage { max_percentage = p; }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            Ok(None) => break, // EOF
                            Err(_) => break,
                        }
                    }
                }
            }
        } else {
            // Fallback for non-IPC (e.g. socket connection failed)
            let _ = child.wait();
        }

        // Ensure clean process exit
        let _ = child.wait();

        // Cleanup
        if std::path::Path::new(&socket_path).exists() {
            let _ = std::fs::remove_file(&socket_path);
        }

        Ok(max_percentage)
    }
}
