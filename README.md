# UHPM - Universal Home Package Manager

[![Rust](https://img.shields.io/badge/Rust-1.70%2B-orange?logo=rust)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

**UHPM** is a high-performance, universal package manager for home use, built in Rust. Designed with speed and flexibility in mind, it handles package installation, version management, and dependency tracking through a robust SQLite backend.

## ✨ Features

- **🚀 Blazing Fast** - Rust-powered core for maximum performance
- **📦 Universal Sources** - Install from repositories, URLs, or local files
- **🔄 Smart Versioning** - Switch between package versions seamlessly
- **🔗 Symbolic Link Management** - Automatic symlink creation with variable expansion
- **🌐 Multi-Repository Support** - Configure multiple package sources
- **📊 SQLite Backend** - Reliable package tracking and metadata storage
- **🎯 Shell Completions** - Bash, Zsh, and Fish autocompletion support
- **🌍 Localization Ready** - Built-in internationalization support

## 🚀 Quick Start

### Installation

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

# Create .uhp archive
uhpmk pack
```

### Database Schema

UHPM maintains three core tables:
- **`packages`** - Package metadata and current version flags
- **`installed_files`** - File-to-package mappings
- **`dependencies`** - Package dependency relationships

### Error Handling

Comprehensive error types cover:
- Configuration errors (`ConfigError`)
- Database operations (`sqlx::Error`)
- Network requests (`reqwest::Error`)
- Package parsing (`MetaParseError`)
- Repository operations (`RepoError`)

## 🎯 Advanced Features

### Multi-Repository Support

Configure repositories in `~/.uhpm/repos.ron`:

```ron
{
    "main": "https://uhpm.example.com/repo",
    "local": "file:///home/user/uhpm-packages",
}
```

### Concurrent Downloads

Package fetcher uses `FuturesUnordered` for parallel downloads with `indicatif` progress bars.

### Localization System

Automatic locale detection with RON-based translation files in `locale/` directory.

## 📚 API Examples

```rust
use uhpm::db::PackageDB;
use uhpm::service::PackageService;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = PackageDB::new("/path/to/packages.db")?.init().await?;
    let service = PackageService::new(db);
    
    // Install package
    service.install_from_repo("example-package", None).await?;
    
    // List packages
    let packages = service.list_packages().await?;
    Ok(())
}
```

## 🗺 Roadmap

- [ ] Remote repository synchronization
- [ ] Dependency resolution improvements
- [ ] Binary delta updates
- [ ] Plugin system for custom installers
- [ ] GPG package verification
- [ ] Traditional package manager hooks

## 🤝 Contributing

Contributions are welcome! Please feel free to submit pull requests or open issues for bugs and feature requests.

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

---

**Note**: UHPM is in active development. Package formats and APIs may change between versions. Check the repository for the latest updates and documentation.
