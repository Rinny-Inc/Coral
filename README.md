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

[resource_pack]
url = ""
hash = "" # optional: sha1sum of the zip for client caching
forced = false
```

## 🤝 Contributing

Coral is in early, fast-moving development. Contributions are welcome, but expect churn.

### Before you start

- Check open [issues](https://github.com/Rinny-Inc/Coral/issues) and the issue templates (Area: Protocol/Packets, World/Chunks, Player/Entity, Commands, Configuration, Performance) before filing a new one — search first to avoid duplicates.
- For anything non-trivial (new systems, protocol handling changes, architectural changes), open an issue or discuss in [Discord](https://discord.com/invite/B2BgjwDX8m) before writing code. Small fixes and bug reports can go straight to a PR.

### Development setup

```bash
git clone https://github.com/Rinny-Inc/Coral.git
cd Coral
cargo build
cargo test
```

Coral is a multi-crate Cargo workspace. Run `cargo check --workspace` before pushing to catch cross-crate breakage.

### Code standards

- **Vanilla accuracy first.** Coral targets Protocol 47 / Minecraft 1.8.x behavior exactly. Approximations, "close enough," or modern-version mechanics are not acceptable — cite vanilla source (decompiled server/client, wiki.vg, or observed vanilla behavior) for anything gameplay-mechanic related (combat, block breaking, movement, etc.).
- Run `cargo fmt` and `cargo clippy --workspace -- -D warnings` before submitting. PRs with clippy warnings will not be merged as-is.
- Keep PRs scoped to one feature/fix. Large multi-feature PRs are harder to review and more likely to get rejected outright.
- No unsafe code without justification in a comment directly above the block.
- Match existing patterns in the codebase (e.g. `PlayerRegistry` for per-player state, `broadcast::Sender` for entity lifecycle events, `Item`/`ItemStack` split for definitions vs. runtime instances) rather than introducing parallel abstractions.

### Commit messages

Use clear, imperative-mood commit messages (`Fix hotbar slot mapping`, not `fixed stuff`). Reference the issue number where applicable (`Fixes #42`).

### Pull requests

- Fork, branch off `main`, PR back into `main`.
- CI (`build.yml`) must pass.
- PRs must follow the [pull request template](.github/PULL_REQUEST_TEMPLATE.md).

### License note

By contributing, you agree your contributions are licensed under the same [custom restrictive license](./LICENSE.md) as the rest of the project.

## 📄 License

This project is licensed under a custom restrictive license.

You are allowed to view, study, and modify this code for personal and educational use. However, you are strictly prohibited from claiming ownership or authorship of this project or any part of its code, whether modified or unmodified.

Any redistribution must include clear attribution to the original project:

> Based on Coral (https://github.com/Rinny-Inc/Coral)

You may not remove or alter copyright notices, nor rebrand this project as your own.

See `LICENSE.md` for full terms.
