# UHPM - Universal Home Package Manager 🚀

**UHPM** is a universal package manager for home use, written in Rust. Future plans include support for application distribution similar to brew.

## Features ✨

- **High performance** (Rust implementation) ⚡
- **Universal** - install packages from files and repositories 📦
- **Version management** - switch between package versions 🔄
- **Developer tools** - create and build packages 🛠️
- **Autocompletion** - generate shell completion scripts 🐚



## Quick Start 🚀

### Install package from repository
`uhpm install package-name`

### Install package from file
`uhpm install -f ./package.uhp`
or
`uhpm install --file ./package.uhp`

### Package Management 📊
List installed packages:
`uhpm list`

Update package from repository:
`uhpm update package-name`

Update package from file:
`uhpm update -f package-name`

Remove package:
`uhpm remove package-name`

Search packages:
`uhpm search query`

### Package Creation (for developers) 👨‍💻
Initialize new package:
`uhpmk init my-package`

Build package:
`uhpmk build`

Create .uhp archive:
`uhpmk pack`

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

This project is licensed under the MIT License.
