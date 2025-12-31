# ani-l

[![Crates.io](https://img.shields.io/crates/v/ani-l.svg)](https://crates.io/crates/ani-l)
[![License: LGPL-2.1](https://img.shields.io/badge/License-LGPL_2.1-blue.svg)](LICENSE)
[![Rust CI](https://github.com/komposer-aml/ani-l/actions/workflows/ci.yml/badge.svg)](https://github.com/komposer-aml/ani-l/actions)

**ani-l** is a terminal-based anime library and streamer inspired by [viu-media/viu](https://github.com/viu-media/viu).

It allows you to browse, search, and stream anime directly from your terminal using a TUI (Text User Interface) or CLI commands.

## ✨ Features

- 🖥️ **TUI Interface**: A clean, keyboard-driven interface built with `ratatui`.
- 🔍 **Search**: Query the AniList API for anime metadata.
- 📺 **Streaming**: Stream episodes directly from sources like AllAnime.
- 💾 **Library Management**: (Coming Soon) Track your watch progress.

## 🛠️ Prerequisites

**You must have `mpv` installed on your system.** `ani-l` delegates video playback to it.

- **macOS**: `brew install mpv`
- **Linux (Debian/Ubuntu)**: `sudo apt install mpv`
- **Windows**: [Download mpv](https://mpv.io/installation/) and ensure it is in your System PATH.

## 📦 Installation

Ensure you have Rust and Cargo installed. Then run:

```bash
cargo install ani-l
```
