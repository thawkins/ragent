# Installation and Setup Guide

This guide provides detailed step-by-step instructions for installing and setting up ragent. It includes system requirements, installation procedures for all major platforms, initial configuration, and troubleshooting common setup issues.

## Table of Contents

1. [System Requirements](#system-requirements)
2. [Prerequisites](#prerequisites)
3. [Installation Methods](#installation-methods)
   - [Installing from Source](#installing-from-source)
   - [Using Pre-built Binaries](#using-pre-built-binaries)
   - [Platform-Specific Installation](#platform-specific-installation)
     - [Windows](#windows)
     - [macOS](#macos)
     - [Linux](#linux)
4. [Initial Configuration](#initial-configuration)
   - [Provider Setup](#provider-setup)
   - [Configuration File](#configuration-file)
5. [First-Time Setup Tips](#first-time-setup-tips)
6. [Troubleshooting](#troubleshooting)

## Prerequisites

Before installing ragent, ensure your system meets the following requirements:

### Minimum System Requirements

- **Operating System**: 
  - Windows 10/11 (64-bit)
  - macOS 11.0 (Big Sur) or later
  - Linux: Ubuntu 20.04+, Debian 11+, Fedora 35+, CentOS 8+, or any modern Linux distribution
- **Rust**: Version 1.85 or higher (edition 2024)
- **Disk Space**: At least 100MB of free space for the binary and dependencies
- **Memory**: Minimum 512MB RAM recommended (1GB+ for larger projects)
- **Internet Connection**: Required for downloading dependencies and LLM provider access

### Hardware Recommendations

- **CPU**: Modern multi-core processor recommended for faster compilation
- **RAM**: 2GB+ for smooth operation with multiple agents
- **Storage**: SSD recommended for faster file operations
- **Network**: Stable internet connection for LLM provider access

## Installation Methods

### Installing from Source

To install ragent from source, follow these steps:

1. Clone the repository:
   ```bash
   git clone https://github.com/thawkins/ragent.git
   cd ragent
   ```

2. Verify your Rust version:
   ```bash
   rustc --version
   cargo --version
   ```
   
   You need Rust 1.85+ (edition 2024). If you need to update:
   ```bash
   rustup update
   rustup self update
   ```

3. Build the project:
   ```bash
   cargo build --release
   ```
   
   **Note**: The first build may take 10-20 minutes depending on your system. Subsequent builds will be faster.

4. Locate the binary:
   The compiled binary will be at `target/release/ragent`.

5. (Optional) Add to PATH:
   
   **Linux/macOS**:
   ```bash
   cp target/release/ragent ~/.local/bin/
   # Or for system-wide installation:
   sudo cp target/release/ragent /usr/local/bin/
   ```
   
   **Windows**: Add `C:\Users\YourUsername\Projects\ragent\target\release\` to your PATH environment variable.

### Using Pre-built Binaries

Pre-built binaries are available for various platforms on the [GitHub releases page](https://github.com/thawkins/ragent/releases).

1. Download the appropriate binary for your platform:
   - Windows: `.exe` installer or `.zip` archive
   - macOS: `.pkg` installer or `.dmg` file
   - Linux: `.deb`, `.rpm`, or `.tar.gz` archive

2. Extract the archive (if applicable):
   ```bash
   # Linux/macOS
   tar -xzf ragent-*.tar.gz
   cd ragent-*
   ```

3. Move the binary to a directory in your PATH:
   
   **Linux/macOS**:
   ```bash
   sudo cp ragent /usr/local/bin/
   ```
   
   **Windows**: Copy `ragent.exe` to a directory in your PATH (e.g., `C:\Program Files\ragent\`).

### Platform-Specific Installation

#### Windows

##### Option 1: Using Cargo (Recommended for Developers)

```powershell
# Install Rust via rustup (requires PowerShell)
irm https://sh.rustup.rs -usep | iex

# Restart your terminal, then:
cargo install --git https://github.com/thawkins/ragent.git
```

##### Option 2: Manual Installation from Source

```powershell
# Clone repository
git clone https://github.com/thawkins/ragent.git
cd ragent

# Build (requires Visual Studio Build Tools or Visual Studio 2019+)
cargo build --release

# Copy binary to a directory in your PATH
Copy-Item target\release\ragent.exe $env:USERPROFILE\.cargo\bin\
```

##### Option 3: Using Chocolatey

```powershell
# Install Chocolatey if not already installed
Set-ExecutionPolicy Bypass -Scope Process -Command 'iex ((New-Object System.Net.WebClient).DownloadString("https://community.chocolatey.org/install.ps1"))'

# Install ragent
choco install ragent
```

##### Windows Troubleshooting

- **Build errors**: Install Visual Studio Build Tools with C++ development tools
- **PATH issues**: Open a new PowerShell window after adding to PATH
- **Antivirus blocking**: Add `target/` directory to antivirus exclusions

#### macOS

##### Option 1: Using Cargo (Recommended)

```bash
# Install Rust via rustup (macOS native installer)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"

# Build ragent
git clone https://github.com/thawkins/ragent.git
cd ragent
cargo build --release
cp target/release/ragent /usr/local/bin/
```

##### Option 2: Using Homebrew (Package Manager)

```bash
# Install Homebrew if not already installed
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

# Install ragent
brew tap thawkins/ragent
brew install ragent
```

##### Option 3: Using Pre-built Binary

```bash
# Download the .pkg installer from GitHub releases
# Double-click to install, or use:
sudo installer -pkg ragent-*.pkg -target /
```

##### macOS Troubleshooting

- **Notarization issues**: Right-click → Open in Finder to bypass Gatekeeper
- **Command line tools**: Install with `xcode-select --install`
- **ARM64 (M1/M2) compatibility**: Ensure you're using Rust ARM64 builds

#### Linux

##### Option 1: Using Cargo (Universal)

```bash
# Install Rust via rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"

# Build ragent
git clone https://github.com/thawkins/ragent.git
cd ragent
cargo build --release
sudo cp target/release/ragent /usr/local/bin/
```

##### Option 2: Distribution-Specific Packages

**Ubuntu/Debian**:
```bash
# Download .deb package from GitHub releases
wget https://github.com/thawkins/ragent/releases/download/v*/ragent_*.deb
sudo dpkg -i ragent_*.deb
sudo apt-get install -f  # Fix dependencies if needed
```

**Fedora/RHEL/CentOS**:
```bash
# Download .rpm package from GitHub releases
wget https://github.com/thawkins/ragent/releases/download/v*/ragent_*.rpm
sudo dnf install ragent_*.rpm
# Or for CentOS/RHEL:
# sudo yum install ragent_*.rpm
```

**Arch Linux**:
```bash
# Using AUR helper (e.g., yay)
yay -S ragent

# Or manually:
git clone https://aur.archlinux.org/ragent.git
cd ragent
makepkg -si
```

##### Linux Troubleshooting

- **Missing dependencies**: Install build essentials:
  ```bash
  # Ubuntu/Debian
  sudo apt-get install build-essential pkg-config libssl-dev
  
  # Fedora/RHEL
  sudo dnf install gcc-c++ pkgconfig openssl-devel
  ```
  
- **Library path issues**: Ensure `/usr/local/bin` is in your PATH:
  ```bash
  echo 'export PATH="/usr/local/bin:$PATH"' >> ~/.bashrc
  source ~/.bashrc
  ```
  
- **Snap/Flatpak conflicts**: If using containerized Rust, build outside containers

## Initial Configuration

## Initial Configuration

### Provider Setup

Ragent requires at least one LLM provider to function. You can configure providers through the interactive TUI or via environment variables.

#### Interactive TUI Setup

1. Launch ragent:
   ```bash
   ragent
   ```

2. Press 'p' on the home screen to open the provider setup dialog.

3. Follow the prompts to:
   - Select a provider (Anthropic, OpenAI, GitHub Copilot, or Ollama)
   - Enter your API key (if required)
   - Choose a model from the provider's available models

The API key will be stored persistently in `~/.local/share/ragent/ragent.db` so you only need to configure once.

#### Environment Variable Setup

You can also configure providers using environment variables:

##### Anthropic (Claude)
```bash
export ANTHROPIC_API_KEY="sk-ant-..."
# or store persistently:
ragent auth anthropic sk-ant-your-key-here
```

##### OpenAI (GPT-4o)
```bash
export OPENAI_API_KEY="sk-..."
# or store persistently:
ragent auth openai sk-your-key-here
```

##### GitHub Copilot
If you have an active GitHub Copilot subscription and the extension installed in VS Code or JetBrains, ragent will auto-discover your Copilot token. No configuration needed.

```bash
# Just works if Copilot is configured in your IDE
ragent run --model copilot/gpt-4o "Explain this code"

# Or set the token explicitly
export GITHUB_COPILOT_TOKEN="ghu_your_token_here"
```

##### Ollama (Local)
```bash
# Install Ollama: https://ollama.com/download
ollama serve          # Start the server
ollama pull llama3.2  # Pull a model

# Ragent auto-detects Ollama at localhost:11434.
# For a remote server, set:
export OLLAMA_HOST="http://your-server:11434"
```

### Configuration File

Ragent can also be configured using a `ragent.json` file in your project directory or at `~/.config/ragent/config.json`. Here's an example configuration:

```json
{
  "provider": {
    "generic_openai": {
      "env": ["GENERIC_OPENAI_API_KEY"],
      "api": { "base_url": "http://127.0.0.1:8080" }
    }
  }
}
```

## First-Time Setup Tips

1. **Start Simple**: Begin with a single provider like Ollama if you want to test locally without API costs.

2. **Explore the TUI**: Once configured, explore the Terminal UI features:
   - Press '?' on an empty input to see keybindings
   - Right-click context menus are available for input and message panels
   - Use Alt+V to attach images from clipboard or file URIs

3. **Try Basic Commands**: Test functionality with simple commands:
   ```bash
   # Launch the interactive TUI
   ragent
   
   # Run a one-shot prompt
   ragent run "Explain this codebase"
   
   # Start the HTTP server only
   ragent serve --port 9100
   ```

4. **Check Health Status**: Look for the health indicator next to the provider on both the home and chat screens:
   - **●** (green) — provider is reachable
   - **✗** (red) — provider is unreachable
   - **●** (yellow) — health check in progress

5. **Configure Permissions**: Review the permissions system to understand how ragent gates file writes, shell commands, and external access. You can auto-approve all permissions with the `--yes` flag, but use with caution.

## Troubleshooting

1. **Build Issues**: If you encounter build errors, ensure you have Rust 1.85+ installed. Update with:
   ```bash
   rustup update
   ```

2. **Provider Connectivity**: If a provider shows as unreachable:
   - Verify your API key is correct
   - Check internet connectivity
   - For Ollama, ensure the server is running (`ollama serve`)

3. **Permission Errors**: If tools fail due to permissions:
   - Check your `ragent.json` configuration
   - Use `--yes` flag to auto-approve permissions (use with caution)

4. **Performance Issues**: If experiencing slow responses:
   - Check your LLM provider's status page
   - For local Ollama models, ensure sufficient system resources

![Screenshot: Provider Setup Dialog](screenshots/provider_setup.png)
*Figure 1: Provider setup dialog in the TUI*

![Screenshot: Main TUI Interface](screenshots/main_interface.png)
*Figure 2: Main TUI interface showing chat panel and log panel*

For additional help, refer to the [Quick Start Guide](QUICKSTART.md) or check the [GitHub repository](https://github.com/thawkins/ragent) for community support.