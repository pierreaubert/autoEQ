<!-- markdownlint-disable-file MD013 -->

# AutoEQ Cross-Platform Build Guide

This guide explains how to build AutoEQ CLI tools for macOS, Linux, and Windows platforms.
It is for developers only. The [README](README.md) contains a simpler way to install the software.

**Note:** For the desktop application build instructions, see [autoeq-app](https://github.com/pierreaubert/autoeq-app).

## Quick Start

### Prerequisites

1. **Rust toolchain** (install via [rustup.rs](https://rustup.rs/))
2. **Docker** (for Linux/Windows cross-compilation)

### Lucky try

Use the provided build script:

```bash
# Make the script executable
chmod +x ./scripts/build-cross.sh

# Run the build
./scripts/build-cross.sh
```

This will create a `dist/` directory with binaries for all supported platforms.

## Supported Platforms

### CLI Tools

- ✅ **macOS ARM64** (Apple Silicon) - `aarch64-apple-darwin`
- ✅ **macOS Intel** - `x86_64-apple-darwin`
- 🐳 **Linux x86_64** - `x86_64-unknown-linux-gnu` (requires Docker)
- ✅ **Linux ARM64** - `aarch64-unknown-linux-gnu` (requires Docker)
- 🐳 **Windows x86_64** - `x86_64-pc-windows-gnu` (requires Docker)

## Manual Build Instructions

### Install Rust Targets

```bash
# Install cross-compilation targets
rustup target add x86_64-apple-darwin aarch64-apple-darwin
rustup target add x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu
rustup target add x86_64-pc-windows-gnu
```

### Install Cross-Compilation Tools

For Linux/Windows builds, install the `cross` tool:

```bash
cargo install cross --git https://github.com/cross-rs/cross
```

### Build CLI Tools

#### Native Build (Current Platform)

```bash
cargo build --release
```

#### macOS Cross-Compilation

```bash
# Intel macOS
cargo build --release --target x86_64-apple-darwin

# Apple Silicon macOS
cargo build --release --target aarch64-apple-darwin
```

#### Linux/Windows (with Docker)

```bash
# Linux x86_64
cross build --release --target x86_64-unknown-linux-gnu

# Linux ARM64
cross build --release --target aarch64-unknown-linux-gnu

# Windows x86_64
cross build --release --target x86_64-pc-windows-gnu
```

## GitHub Actions (Automated Builds)

The repository includes a GitHub Actions workflow (`.github/workflows/build.yml`) that automatically builds binaries for all platforms on:

- Push to main/master branch
- Pull requests
- Git tags (creates releases)

## Output Structure

After building, you'll find binaries in:

```text
dist/
├── aarch64-apple-darwin/          # macOS Apple Silicon
│   ├── autoeq                     # Main CLI tool
│   ├── download                   # Data download tool
│   ├── benchmark                  # Performance testing
│   └── README.txt                 # Build info
├── x86_64-apple-darwin/           # macOS Intel
├── x86_64-unknown-linux-gnu/      # Linux x86_64
├── aarch64-unknown-linux-gnu/     # Linux ARM64
└── x86_64-pc-windows-gnu/         # Windows x86_64
    ├── autoeq.exe
    ├── download.exe
    └── benchmark.exe
```

## Troubleshooting

### Docker Issues

- Ensure Docker is running: `docker ps`
- Update cross tool: `cargo install cross --git https://github.com/cross-rs/cross --force`

### macOS Builds

- May require Xcode Command Line Tools: `xcode-select --install`
- Ensure both Intel and ARM targets are installed

### Windows MinGW Issues

- Cross-compilation may require additional MinGW toolchain setup
- Consider using GitHub Actions for Windows builds

## Testing Binaries

```bash
# Test CLI tool
./dist/aarch64-apple-darwin/autoeq --help

# Test with sample data
./dist/aarch64-apple-darwin/autoeq --speaker="KEF R3" --version=asr --measurement=CEA2034

# Verify architecture
file dist/x86_64-apple-darwin/autoeq
file dist/aarch64-apple-darwin/autoeq
```

## Distribution

The build script creates compressed archives:

- `autoeq-{target}.tar.gz` for Unix platforms
- `autoeq-{target}.zip` for Windows

These are ready for distribution or release uploads.

## Next Steps

1. Fix TypeScript issues in UI code for Tauri builds
2. Test binaries on actual target platforms
3. Set up automated testing in CI/CD
4. Create proper release packaging with version numbers
5. Add code signing for macOS/Windows distributables
