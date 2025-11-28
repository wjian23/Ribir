# Installation

To start developing with Ribir, you need to have the Rust toolchain installed.

## Prerequisites

### 1. Rust Toolchain
Ribir requires a stable Rust installation. If you haven't installed Rust yet, you can do so via [rustup](https://rustup.rs/):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### 2. Platform Dependencies

#### Windows
Ensure you have the C++ build tools installed (Visual Studio Build Tools).

#### Linux
You may need to install development libraries for core system components (like X11 or Wayland, ALSA, etc., depending on the backend).

For Debian/Ubuntu based systems:

```bash
sudo apt-get install libx11-dev libxkbcommon-dev libwayland-dev libfontconfig1-dev
```

#### macOS
Xcode Command Line Tools are required.
```bash
xcode-select --install
```

## Adding Ribir to your Project

Create a new binary project:
```bash
cargo new my_ribir_app
cd my_ribir_app
```

Add `ribir` as a dependency in your `Cargo.toml`:

```toml
[dependencies]
ribir = "0.1" # Check crates.io for the latest version
```

Or use `cargo add`:
```bash
cargo add ribir
```
