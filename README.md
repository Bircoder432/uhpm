# UHPM - Universal Home Package Manager 🚀

**UHPM** is a universal package manager for home use, written in Rust. Future plans include support for application distribution similar to brew.

## Features ✨

- **High performance** (Rust implementation) ⚡
- **Universal** - install packages from files and repositories 📦
- **Version management** - switch between package versions 🔄
- **Developer tools** - create and build packages 🛠️
- **Autocompletion** - generate shell completion scripts 🐚

- **🚀 Blazing Fast** - Rust-powered core for maximum performance
- **📦 Universal Sources** - Install from repositories, URLs, or local files
- **🔄 Smart Versioning** - Switch between package versions seamlessly
- **🔗 Symbolic Link Management** - Automatic symlink creation with variable expansion
- **🌐 Multi-Repository Support** - Configure multiple package sources
- **📊 SQLite Backend** - Reliable package tracking and metadata storage
- **🎯 Shell Completions** - Bash, Zsh, and Fish autocompletion support
- **🌍 Localization Ready** - Built-in internationalization support

## 🚀 Quick Start

## Quick Start 🚀

```bash
# Build from source
git clone https://github.com/bircoder432/uhpm.git
cd uhpm
cargo build --release
```

### Basic Usage

```bash
# Install from repository
uhpm install package-name

# Install from local file
uhpm install -f ./package.uhp

# List installed packages
uhpm list

# Update a package
uhpm update package-name

# Remove package
uhpm remove package-name

# Switch package version
uhpm switch package-name@1.2.3
```

## 📁 Project Structure

```
~/.uhpm/
├── config.ron          # Configuration file
├── packages.db         # SQLite package database
├── packages/           # Installed package versions
│   └── package-version/
├── tmp/               # Temporary files
└── repos.ron          # Repository configurations
```

## 🛠 Core Architecture

### Key Modules

- **`config`** - Configuration management with RON serialization
- **`db`** - SQLite-based package database with version tracking
- **`fetcher`** - Parallel package downloading with progress bars
- **`package`** - Package metadata and installation logic
- **`symlist`** - Symbolic link management with environment variables
- **`repo`** - Repository management and package discovery
- **`service`** - High-level package operations API

### Package Format

Packages use TOML metadata and support:

```toml
name = "my_package"
author = "Developer"
version = "1.0.0"
checksum = "sha256:abc123"

[src]
type = "Url"
value = "https://example.com/package.uhp"

[[dependencies]]
name = "required_dep"
version = "1.0.0"
```

### Symbolic Link Management

UHPM uses `symlist` files to manage symbolic links with variable expansion:

```bash
# symlist format: source_path target_path_with_variables
bin/my_tool $HOME/.local/bin/my_tool
config/app.conf $XDG_CONFIG_HOME/app.conf
```

Supported variables: `$HOME`, `$XDG_DATA_HOME`, `$XDG_CONFIG_HOME`, `$XDG_BIN_HOME`

## 🔧 Development

### Package Creation

```bash
# Initialize new package
uhpmk init

# Build package
uhpmk build

## Project Structure 🏗️

The project consists of two main components:
- **uhpm** - main client for package management
- **uhpmk** - utility for package development and building

## Available Commands ⌨️

### Main commands (uhpm)
- `install` - Install package from repository
- `install -f/--file` - Install package from file
- `remove` - Remove installed packages 🗑️
- `list` - List installed packages 📋
- `self-remove` - Remove UHPM from system
- `update` - Update package from repository
- `update -f/--file` - Update package from file
- `switch` - Switch active package version
- `completions` - Search packages and generate autocompletion scripts

### Development commands (uhpmk)
- `init` - Initialize new package template
- `build` - Build package using build script
- `pack` - Package directory into .uhp archive

## Development 🔧

### Build from source
```bash
git clone https://github.com/bircoder432/uhpm.git
cd uhpm
cargo build --release
```

### Development installation


## Roadmap 🗺️

- [ ] Package repository support
- [ ] Multi-architecture and OS support
- [ ] Traditional package manager hooks

## License 📄

**Note**: UHPM is in active development. Package formats and APIs may change between versions. Check the repository for the latest updates and documentation.
