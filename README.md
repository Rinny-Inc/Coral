# 🦀🪸 Coral

> ⚠️ Early development — not production ready

![Rust](<https://img.shields.io/badge/Rust-stable%20(MSRV)-orange?logo=rust>)

![Rust Lines of Code](https://img.shields.io/badge/Lines%20of%20code-8490-orange)<!-- LOC_BADGE_START --><!-- LOC_BADGE_END -->

![Minecraft](https://img.shields.io/badge/Minecraft-1.8.9-green)
![Protocol](https://img.shields.io/badge/Protocol-47-blue)
[![Build](https://github.com/Rinny-Inc/Coral/actions/workflows/build.yml/badge.svg)](https://github.com/Rinny-Inc/Coral/actions/workflows/build.yml)
![Status](https://img.shields.io/badge/Status-In%20Development-yellow)
[![Discord](https://img.shields.io/discord/1407520428947603527?label=Discord&logo=discord)](https://discord.com/invite/B2BgjwDX8m)
![License](https://img.shields.io/badge/License-Custom%20Restrictive-red)

> ## 💬 Discord Community
>
> **Join the Coral Discord server for development updates, support, bug reports, and discussions.**
>
> ### https://discord.gg/invite/B2BgjwDX8m

**Coral** is a high-performance, lightweight legacy Minecraft server implementation written entirely in Rust. Coral targets **Minecraft 1.8.9 (Protocol 47)** as its primary platform while laying the foundation for **Minecraft 1.7.10 compatibility** through native multi-version protocol support.

Built for performance and modern concurrency, Coral aims to provide a familiar Minecraft server experience without the limitations of the traditional JVM server stack.

[Features](#-features) • [Getting Started](#-getting-started) • [Configuration](#%EF%B8%8F-configuration) • [Contributing](#-contributing)

---

## ⚡ Features

- **Memory Safe:** Powered by Rust's strict ownership model, eliminating data races and null pointer crashes.
- **Blazing Fast:** Async networking, lock-efficient systems, and modern Rust performance.
- **1.8.9 First:** Primary target is Minecraft 1.8.9 (Protocol 47).
- **Native Multi-Version Support (Planned):** Designed to support multiple Minecraft protocol versions within a single server implementation.
- **1.7.10 Planned:** Future support for Minecraft 1.7.10 clients and protocol compatibility.
- **Bukkit-Friendly Plugins (Planned):** Planned Java plugin loader with a Bukkit-inspired API for seamless plugin development.
- **Java Plugin Support (Planned):** Dedicated Java plugin loader and runtime integrated directly into Coral.
- **Built From Scratch:** No Spigot, Paper, or CraftBukkit code; a clean-room Rust implementation.

## 🚀 Getting Started

### Prerequisites

You need the stable Rust toolchain installed on your machine. If you don't have it, get it via [rustup.rs](https://rustup.rs):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://rustup.rs | sh
```

### Installation & Build

1. Clone the repository:

   ```bash
   git clone https://github.com/Rinny-Inc/Coral.git
   cd Coral
   ```

2. Build the project in release mode:

   ```bash
   cargo build --release
   ```

3. Run the server:
   ```bash
   cargo run --release
   ```

## ⚙️ Configuration

On the first boot, Coral will generate a `config.toml` config file in the root directory.

```toml
[server]
motd = "Coral Rust Minecraft Server\nTest Server"
port = 25565
max_players = 20
online_mode = true
player_sample_size = 12
default_gamemode = 0
whitelisted = false
view_distance = 10
compression_threshold = 256

[chat]
format = "<{username}> {message}"

[world]
name = "world"
difficulty = 0
item_despawn_seconds = 300
disable_weather = false
allow_nether = true
allow_end = true
enable_auto_save = true
# In Seconds
auto_save_interval = 300

[tracking]
player = 512
mob = 80
item = 64
experience_orb = 64
```

## 🤝 Contributing

Guidelines will be added once the project stabilizes.

## 📄 License

This project is licensed under a custom restrictive license.

You are allowed to view, study, and modify this code for personal and educational use. However, you are strictly prohibited from claiming ownership or authorship of this project or any part of its code, whether modified or unmodified.

Any redistribution must include clear attribution to the original project:

> Based on Coral (https://github.com/Rinny-Inc/Coral)

You may not remove or alter copyright notices, nor rebrand this project as your own.

See `LICENSE.md` for full terms.
