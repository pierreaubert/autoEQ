# WARP.md

This file provides guidance to WARP (warp.dev) when working with code in this repository.

## Project Overview

AutoEQ is a Rust-based automatic equalization system for speakers and headphones that uses global optimization algorithms to find optimal IIR filters. The project integrates with spinorama.org to fetch speaker measurement data and includes both CLI tools and a Tauri-based desktop application (UI).

## Core Architecture

The project uses a workspace structure with specialized crates:

- **src-autoeq**: Main library and CLI tools (autoeq, download, benchmark)
- **src-iir**: IIR filter processing and Biquad implementations
- **src-de**: Differential Evolution optimizer with adaptive strategies
- **src-cea2034**: CEA2034 scoring and metrics computation
- **src-testfunctions**: Test functions for optimization benchmarking
- **src-env**: Environment utilities for test infrastructure
- **src-ui**: Tauri desktop application with TypeScript/Vite frontend

### Data Flow
1. Input: Speaker/headphone frequency response curves (CSV or API)
2. Processing: Optimization algorithms find optimal IIR filter parameters
3. Output: PEQ filters, plots (HTML/PNG), and performance metrics

## Common Development Commands

### Building
```bash
# Build everything in release mode
cargo build --release

# Build specific binary
cargo build --release --bin autoeq

# Cross-platform builds (requires Docker)
chmod +x ./scripts/build-cross.sh
./scripts/build-cross.sh
```

### Testing
```bash
# Run all tests (requires AUTOEQ_DIR environment variable)
export AUTOEQ_DIR=/Users/pierrre/src.local/autoeq
cargo test --workspace --release

# Run specific crate tests
cargo test --package src-iir --release
```

### Linting and Formatting
```bash
# Format code
cargo fmt

# Lint with clippy
cargo clippy
```

### Running the CLI Tools
```bash
# Basic optimization with spinorama.org data
cargo run --bin autoeq --release -- --speaker="KEF R3" --version=asr --measurement=CEA2034 --algo cobyla

# With custom input curve
cargo run --bin autoeq --release -- --curve=input.csv --target=target.csv --n=7 --algo autoeq:de

# Download all speaker data from spinorama.org
cargo run --bin download --release

# Run benchmark across speaker database
cargo run --bin benchmark --release -- --algo cobyla
```

### UI Development
```bash
cd src-ui

# Install dependencies
npm install

# Development server
npm run dev

# Build desktop app
npm run tauri build

# Run tests
npm test
```

## Optimization Algorithms

The system supports multiple optimization strategies:

**Global algorithms with constraints:**
- ISRES, AGS, ORIGDIRECT

**Global with bounds:**
- DE (Differential Evolution with adaptive strategies)
- PSO, STOGO, GMLSL

**Local optimization:**
- cobyla (recommended for speed)
- neldermead

**DE-specific strategies (--strategy flag):**
- currenttobest1bin (default, recommended)
- best1bin, rand1bin (classic)
- adaptivebin, adaptiveexp (experimental)

## Key Parameters

- `--n`: Number of IIR filters (default: 7)
- `--min-q/--max-q`: Q factor bounds (default: 1-3)
- `--min-db/--max-db`: Gain bounds (default: 1-3 dB)
- `--min-freq/--max-freq`: Frequency range (default: 60-16000 Hz)
- `--loss`: Optimization target (flat, score, mixed)
- `--smooth`: Apply smoothing to target curve
- `--refine`: Apply local optimization after global

## Project-Specific Rules

### Version Management
Before committing, increase the version number in:
1. The relevant Cargo.toml file (patch position: e.g., 0.3.24 → 0.3.25)
2. The top-level Cargo.toml file (always increase)

### Data Directory Exclusions
When indexing the codebase, skip:
- data/measurements
- data/picture
- data/eq

### File Restrictions
The meta.js file should not be modified (edited later, causes linter complaints).

## Dependencies and Requirements

- Rust toolchain (via rustup)
- Docker (for cross-compilation)
- Node.js 18+ (for UI)
- Environment variable: `AUTOEQ_DIR` (for tests)

## API Integration

The project integrates with spinorama.org API:
- `/v1/speakers` - List all speakers
- `/v1/speakers/{speaker}/versions` - Get speaker versions
- `/v1/speakers/{speaker}/versions/{version}/measurements` - Get measurements

## Output Files

- **Plots**: HTML and PNG visualizations of frequency responses
- **PEQ Settings**: Console output of filter parameters
- **CSV Traces**: Test infrastructure writes trace files to `$AUTOEQ_DIR`

## Testing Infrastructure

The project includes comprehensive benchmarking tools to test optimization algorithms across the entire speaker database. Results are exported to CSV for analysis.
