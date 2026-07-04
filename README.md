# 🦀🪸 Coral

> ⚠️ Early development — not production ready

<p align="center">
<img alt="Rust" src="https://img.shields.io/badge/Rust-stable%20(MSRV)-orange?logo=rust">
<a href="https://github.com/Rinny-Inc/Coral/actions/workflows/line_counter.yml"><img alt="Rust Lines of Code" src="https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/Rinny-Inc/Coral/badges/line_badge.json"></a>
<img alt="Protocol" src="https://img.shields.io/badge/Protocol-47%20(1.8.x)-blue">
<a href="https://github.com/Rinny-Inc/Coral/actions/workflows/build.yml"><img alt="Build" src="https://github.com/Rinny-Inc/Coral/actions/workflows/build.yml/badge.svg"></a>
<img alt="Status" src="https://img.shields.io/badge/Status-In%20Development-yellow">
<a href="https://discord.com/invite/B2BgjwDX8m"><img alt="Discord" src="https://img.shields.io/discord/1352833901860487299?label=Discord&logo=discord"></a>
<img alt="License" src="https://img.shields.io/badge/License-Custom%20Restrictive-red">
</p>

> ## 💬 Discord Community
>
> **Join the Coral Discord server for development updates, support, bug reports, and discussions.**
>
> ### [Click to join](https://discord.com/invite/B2BgjwDX8m)

**Coral** is a high-performance, lightweight legacy Minecraft server implementation written entirely in Rust. Coral targets **Minecraft 1.8 (Protocol 47)** as its primary platform while laying the foundation for **Minecraft 1.7.10 compatibility** through native multi-version protocol support.

Built for performance and modern concurrency, Coral aims to provide a familiar Minecraft server experience without the limitations of the traditional JVM server stack.

[Planned Features](#-planned-features) • [Getting Started](#-getting-started) • [Configuration](#%EF%B8%8F-configuration) • [Contributing](#-contributing)

---

## ⚡ Planned Features

- **Memory Safe:** Powered by Rust's strict ownership model, eliminating data races and null pointer crashes.
- **1.8 First:** Primary target is Minecraft 1.8 (Protocol 47).
- **Native Multi-Version Support:** Designed to support multiple Minecraft protocol versions within a single server implementation.
- **1.7.10:** Future support for Minecraft 1.7.10 clients and protocol compatibility.
- **Bukkit-Friendly Plugins:** Planned Java plugin loader with a Bukkit-inspired API for seamless plugin development.
- **Java Plugin Support:** Dedicated Java plugin loader and runtime integrated directly into Coral.
- **Built From Scratch:** No Spigot, Paper, or CraftBukkit code; a clean-room Rust implementation.
- **Full Server Authority:** All movement, combat, and inventory actions will be validated server-side; the client is never trusted, closing the door on reach, speed, and packet-manipulation exploits.

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
enforce_default_gamemode = true
whitelisted = false
view_distance = 10
compression_threshold = 256
connection_throttle_ms = 4000

[chat]
format = "<{username}> {message}"

[world]
world_name = "world"
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

[bungee]
enabled = false
addresses = ["127.0.0.1"]
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
