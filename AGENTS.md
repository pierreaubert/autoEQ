# WARP.md

This file provides guidance to WARP (warp.dev) when working with code in this repository.

## Project Overview

AutoEQ is an automatic equalization library and tool for speakers and headphones written in Rust. It finds optimal IIR filter parameters to correct audio frequency response based on measurements from [Spinorama.org](https://spinorama.org). The project includes both command-line tools and a Tauri-based desktop UI.

### Core Architecture

The codebase is organized into three main components:

1. **Core Library** (`src/lib.rs`): Provides the fundamental building blocks
   - `iir.rs`: Biquad IIR filter implementations (lowpass, highpass, peak, shelf)
   - `optim.rs`: Global and local optimization algorithms (NLOPT, Differential Evolution)
   - `loss.rs`: Loss functions (flat, score-based, mixed) for optimization objectives
   - `read.rs`: Data loading from Spinorama.org API and local files
   - `plot.rs`: Visualization using Plotly for frequency responses and filters
   - `score.rs`: Audio preference scoring based on Harman/Olive research
   - `constraints.rs`: Filter spacing and gain constraints for realistic EQ
   - `workflow.rs`: Shared processing steps used across binaries

2. **CLI Binaries** (`bin/`):
   - `autoeq.rs`: Main EQ optimization tool
   - `download.rs`: Bulk data fetching from Spinorama.org
   - `benchmark.rs`: Performance testing across speaker database

3. **Desktop UI** (`ui/`): Tauri app with TypeScript/Vite frontend

## Data Organization

**Important Rule**: All data files must follow this organization pattern:
- **Cached data** (downloaded measurements, API responses): Use `DATA_CACHED` directory defined in `constants.rs` (currently `data_cached/`)
- **Generated data** (plots, analysis results, EQ parameters): Use `DATA_GENERATED` directory (currently `data_generated/`)

This separation ensures clean organization between external data sources and tool outputs.

## Code  Organization
- Code is organized by modules.
- Code for libraries and crates goes into src
- Code for binaries goes into bin
- Code for tests (see below the tests organisation)
- Code is formatted with cargo fmt
- Code is checked with carge check
- Code is linted with cargo clippy

## Code properties

**Important Rules** :
- Code is as simple as possible.
- Code does not have much duplication, factorize common code.
- Size of function is under 100 lines.
- Code is tested.
- Code does not use use wildcard but the specific list of used functions.

## Tests Organization

**Important Rule**:
- **Unit tests** go into the file where the code is. They should be fast to execute.
- **Optimisation accuracy testa**
  - this tests go into the tests directory,
  - this tests are run with --release
  - the function to be evaluated goes into tests/testfunctions/mod.rs.
  - this function are never deleted even if not used at the moment.
  - This function are documented with test attributes and expected best minima.

## Common Development Commands

### Building
```bash
# Build all workspace members (library + binaries + UI)
cargo build --workspace --release

# Build only the main CLI tools (excludes UI)
cargo build --release

# Build specific binary
cargo build --release --bin autoeq
cargo build --release --bin download
cargo build --release --bin benchmark

# Build only the UI
cargo build --release -p autoeq-ui

# Cross-platform builds for distribution
chmod +x ./scripts/build-cross.sh
./scripts/build-cross.sh
```

### Testing

**Important**: Tests that use recording functionality require the `AUTOEQ_DIR` environment variable to be set to the project root directory:

```bash
# Set the environment variable for your shell session
export AUTOEQ_DIR=/path/to/your/autoeq/project

# Or set it just for the test command
AUTOEQ_DIR=/path/to/your/autoeq/project cargo test --lib
```

Test commands:
```bash
# Run lib tests (fast)
cargo test --lib

# Run all tests (slow)
cargo test --release

# Run tests for entire workspace
cargo test --workspace

# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_recording

# Check all workspace members
cargo check --workspace
```

**Note**: The `AUTOEQ_DIR` environment variable is used by the test infrastructure to determine where to write CSV trace files and other generated data. Without it, tests that record optimization traces will fail.

### Running Tools
```bash
# Main EQ optimization
cargo run --bin autoeq --release -- --speaker="KEF R3" --version=asr --measurement=CEA2034 --algo=cobyla

# Download test data (takes 3-5 minutes)
cargo run --bin download --release

# Performance benchmarking
cargo run --bin benchmark --release -- --algo cobyla

# Show available algorithms and parameters
cargo run --bin autoeq --release -- --help
```

### UI Development
```bash
cd ui

# Install dependencies
npm install

# Development mode with hot reload
npm run dev
npm run tauri dev

# Build desktop app
npm run tauri build
```

### Differential Evolution Examples

Note: Some examples and tools that generate data files require `AUTOEQ_DIR` to be set:

```bash
# Set environment variable first
export AUTOEQ_DIR=/path/to/your/autoeq/project

# Run basic DE example
cargo run --example optde_basic

# Run linear constraints example
cargo run --example optde_linear_constraints

# Run nonlinear constraints example
cargo run --example optde_nonlinear_constraints

# Plot function visualizations with optimization traces
cargo run --bin plot_functions --release -- --show_traces
```

## Key Technical Concepts

### Optimization Flow
1. **Input Loading**: Fetch speaker measurements from Spinorama.org or load local data
2. **Target Curve**: Generate reference curve (flat, Harman target, or custom)
3. **Objective Setup**: Configure loss function (flat/score/mixed) and constraints
4. **Bounds Setup**: Set frequency, Q-factor, and gain limits for filters
5. **Global Optimization**: Find initial solution using algorithms like ISRES, DE, PSO
6. **Local Refinement**: Polish with Nelder-Mead or COBYLA if `--refine` enabled
7. **Output**: Generate PEQ parameters and visualization plots

### Filter Architecture
The project uses biquad IIR filters with these types:
- **Peak (PK)**: Most common, adjusts specific frequency bands
- **Low/High Shelf (LS/HS)**: Broad corrections at frequency extremes
- **Low/High Pass (LP/HP)**: Removes content below/above cutoff

Parameters are encoded as: `[freq1, q1, gain1, freq2, q2, gain2, ...]`

### Loss Functions
- **Flat**: Minimize RMS error against target curve (good for nearfield)
- **Score**: Maximize Harman preference score (good for room listening)
- **Mixed**: Balance LW and PIR flatness (experimental hybrid approach)

### Constraint System
- **Frequency bounds**: `--min-freq` to `--max-freq` (default 20Hz-20kHz)
- **Q factor limits**: `--min-q` to `--max-q` (sharpness control)
- **Gain limits**: `--min-db` to `--max-db` (boost/cut range)
- **Filter spacing**: Prevents overlapping filters via `--min-spacing-oct`

## API Integration

Data comes from the Spinorama.org REST API:
```bash
# List all speakers
curl http://api.spinorama.org/v1/speakers

# Get versions for a speaker
curl "http://api.spinorama.org/v1/speakers/KEF R3/versions"

# Get measurements for speaker/version
curl "http://api.spinorama.org/v1/speakers/KEF R3/versions/asr/measurements"
```

## Performance and Algorithms

### Global Optimizers (good exploration, slower)
- **ISRES**: Evolutionary strategy with constraints (recommended for most cases)
- **DE**: Differential evolution (custom Rust implementation)
- **PSO**: Particle swarm optimization
- **AGS**: Adaptive global search

### Local Optimizers (fast refinement)
- **COBYLA**: Constrained optimization (recommended for refinement)
- **NELDERMEAD**: Simplex method (fast, no constraints)

Typical workflow: Global → Local refinement with `--refine`

## File Locations and Build Artifacts

### Development Structure
- `src/`: Core Rust library modules
  - `src/iir/`: IIR filter implementations and utilities
  - `src/de/`: Differential Evolution optimizer with tests and examples
- `bin/`: CLI tool entry points
- `ui/`: Tauri desktop application
- `scripts/`: Build and utility scripts
- `data_tests/`, `tests/`: Testing and examples

### Build Outputs
- Native: `target/release/{autoeq,download,benchmark}`
- Cross-platform: `dist/{platform}/{binaries}` via build script
- UI bundles: `ui/src-tauri/target/{platform}/release/bundle/`

## Dependencies

### Core Libraries
- **ndarray + BLAS**: High-performance linear algebra
- **nlopt**: Global/local optimization algorithms
- **plotly**: Interactive frequency response plots
- **reqwest + tokio**: Async HTTP client for API calls
- **serde**: JSON serialization for API data

### UI Stack
- **Tauri 2.0**: Rust backend with web frontend
- **TypeScript + Vite**: Frontend build system
- **Plotly.js**: Interactive charts in the UI

## Debugging Tips

### Signal Handling
Both `autoeq` and `benchmark` support graceful Ctrl+C shutdown. Optimization will complete the current iteration before stopping.

### Constraint Diagnostics
The tool shows filter spacing diagnostics with ✅/⚠️ indicators to help debug constraint violations.

### Optimization Traces
Use custom evaluation recorder in `evaluation_recorder.rs` to trace optimization progress and diagnose convergence issues.

### Common Issues
- **Docker required** for Linux/Windows cross-compilation via `cross`
- **API rate limits** when downloading large datasets
- **Memory usage** scales with population size in global optimizers
- **TypeScript errors** in UI require fixes before Tauri builds work

# Version Increment Rule

**Important Rule**: Before committing any changes, increment the version number in the relevant Cargo.toml file(s) by 1 in the patch version (the third number in semantic versioning).

## Version Increment Guidelines

### For Individual Crate Changes
- If changes are made to a specific crate (e.g., `src-de/`, `src-iir/`, `src-cea2034/`, etc.), increment the version in that crate's `Cargo.toml`
- Example: `0.2.31` becomes `0.2.32`

### For Main Application Changes
- If changes are made to the main `autoeq` crate (`src-autoeq/`), increment the version in `src-autoeq/Cargo.toml`
- Example: `0.2.190` becomes `0.2.191`

### For Workspace-Wide Changes
- If changes affect multiple crates or workspace-level configuration, increment the version in the main application crate (`src-autoeq/Cargo.toml`) as it represents the primary deliverable
- Also increment versions in any directly modified crate's `Cargo.toml`

### Version Format
- Follow semantic versioning: `MAJOR.MINOR.PATCH`
- For regular development: increment PATCH (third number)
- Example transformations:
  - `0.3.24` → `0.3.25`
  - `0.2.190` → `0.2.191`
  - `0.2.31` → `0.2.32`

### Implementation
1. Before running `git commit`, check which crates were modified
2. Increment the appropriate version number(s) in the corresponding `Cargo.toml` file(s)
3. Include the version bump in the same commit as the changes
4. Use descriptive commit messages that mention the version increment

This ensures proper version tracking and helps with release management and dependency resolution across the workspace.
