# ani-l

[![Crates.io](https://img.shields.io/crates/v/ani-l.svg)](https://crates.io/crates/ani-l)
[![License: LGPL-2.1](https://img.shields.io/badge/License-LGPL_2.1-blue.svg)](LICENSE)
[![Rust CI](https://github.com/komposer-aml/ani-l/actions/workflows/ci.yml/badge.svg)](https://github.com/komposer-aml/ani-l/actions)

**ani-l** is a terminal-based anime library and streamer inspired by [viu-media/viu](https://github.com/viu-media/viu).

It allows you to browse, search, and stream anime directly from your terminal using a TUI (Text User Interface) or CLI commands.

<p align="center">
  <a href="https://discord.gg/CTkxHNvHRy">
    <img src="http://invidget.switchblade.xyz/CTkxHNvHRy" alt="Discord Server Invite">
  </a>
</p>

> [!IMPORTANT]
> This project scrapes public-facing websites for its streaming / downloading capabilities and primarily acts as an anilist, jikan and many other media apis tui client. The developer(s) of this application have no affiliation with these content providers. This application hosts zero content and is intended for educational and personal use only. Use at your own risk.
>
> [**Read the Full Disclaimer**](DISCLAIMER.md)

## ✨ Features

- 🖥️ **TUI Interface**: A clean, keyboard-driven interface built with `ratatui`.
- 🔍 **Search**: Query the AniList API for anime metadata.
- 📺 **Streaming**: Stream episodes directly from sources like AllAnime.
- 💾 **Library Management**: Track your watch progress.

## 📦 Installation

### 🛠️ Prerequisites

#### Installing Rust

- **MacOS/Linux**: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`

- **Windows**: Use [rustup](https://rustup.rs/)

#### Installing `mpv`

**You must have `mpv` installed on your system.** `ani-l` delegates video playback to it.

- **macOS**: `brew install mpv`
- **Linux (Debian/Ubuntu)**: `sudo apt install mpv`
- **Windows**: [Download mpv](https://mpv.io/installation/) and ensure it is in your System PATH.

#### Option A: Install from Crates.io (Recommended)

```bash
cargo install ani-l
```

#### Option B: Build from Source

Clone the repository:

```bash
git clone [https://github.com/komposer-aml/ani-l.git](https://github.com/komposer-aml/ani-l.git)
cd ani-l
```

Build and install:

```bash
cargo install --path .
```

## 🚀 Usage

### TUI Mode (Default)

Simply run the command to enter the interactive interface:

```bash
ani-l
```

#### Keybindings

| Key             | Action                |
| :-------------- | :-------------------- |
| /               | Focus Search Bar      |
| Enter           | Select / Search       |
| j / Down        | Move Down             |
| k / Up          | Move Up               |
| J / PgDn        | Jump Down (10 items)  |
| K / PgUp        | Jump Up (10 items)    |
| Esc / Backspace | Go Back / Cancel      |
| q               | Quit (from Main Menu) |

#### CLI Commands

You can also use ani-l directly from the command line without the TUI.
Search for an Anime:

```bash
ani-l search query --text "Naruto"
```

View Trending Anime:

```bash
ani-l search trending --page 1
```

Play a specific URL:

```bash
ani-l play --url "[https://example.com/video.mp4](https://example.com/video.mp4)" --title "My Video"
```

Watch a specific episode (CLI Stream):

```bash
# Searches and attempts to stream Episode 1 automatically
ani-l watch --query "One Piece" --episode 1
```

### ⚙️ Configuration

`ani-l` stores configuration files in your system's standard config directory:

• Linux: `~/.config/ani-l/config.toml`

• macOS: `~/Library/Application Support/com.sleepy-foundry.ani-l/config.toml`

• Windows: `C:\Users\You\AppData\Roaming\sleepy-foundry\ani-l\config.toml`

Example config.toml:

```toml
[general]
provider = "allanime"

[stream]
player = "mpv"
quality = "1080"
translation_type = "sub"
episode_complete_at = 85
```

## 👾 Contribution Guide

Contributions are welcome!

1. Fork the repository.
2. Clone your fork: `git clone https://github.com/YOUR_USERNAME/ani-l.git`
3. Create a new Branch: `git checkout -b type/issue_id-short_description`
4. Commit your changes: `git commit -m 'feat(scope): Added some amazing feature'`
5. Push to the branch: `git push origin feat/123-amazing-feature`
6. Open a Pull Request.

### Development Guidelines

- Ensure your code is formatted: `cargo fmt`
- Check for lints: `cargo clippy`
- Run tests: `cargo test`

## 📄 License

This project is licensed under the LGPL-2.1 License.
