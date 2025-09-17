# AutoEQ - Automatic Equalization

The main AutoEQ crate providing automatic equalization for speakers and headphones using measurement data from Spinorama.org.

## Features

- **Automatic Filter Optimization**: Find optimal IIR filter parameters to match target curves
- **Multiple Loss Functions**: Flat response, preference score, or mixed optimization
- **Advanced Constraints**: Filter spacing, frequency bounds, and gain limits
- **Data Integration**: Fetch measurements directly from Spinorama.org API
- **Visualization**: Generate interactive plots of frequency responses and filters
- **Benchmarking**: Comprehensive performance testing across speaker database

## Core Applications

### AutoEQ CLI Tool
The main equalization application:
```bash
# Optimize KEF R3 using CEA2034 data with score-based loss
autoeq --speaker "KEF R3" --version asr --measurement CEA2034 \
       --loss score --algo nlopt:isres --refine

# Generate 5-band PEQ for headphones with custom target
autoeq --curve my_headphone_measurement.csv --target harman_target.csv \
       --num-filters 5 --loss flat --algo autoeq:de
```

### Benchmark Tool
Performance testing across speaker database:
```bash
# Run benchmark across all cached speakers
benchmark --algo nlopt:isres

# Quick smoke test with first 5 speakers
benchmark --smoke-test --jobs 4
```

### Download Tool
Bulk data fetching from Spinorama.org:
```bash
# Download CEA2034 measurements for all speakers
download
```

## Architecture

### Optimization Pipeline
1. **Data Loading**: Fetch measurements from API or load local files
2. **Target Generation**: Create reference curve (flat, Harman, custom)
3. **Objective Setup**: Configure loss function and constraints
4. **Global Search**: Find initial solution using DE, PSO, or NLOPT
5. **Local Refinement**: Polish result with Nelder-Mead or COBYLA
6. **Validation**: Check filter spacing and constraint satisfaction

### Loss Functions
- **Flat Loss**: Minimize RMS error against target curve
- **Score Loss**: Maximize CEA2034-based preference score
- **Mixed Loss**: Combine listening window and PIR optimization

### Optimization Algorithms
- **NLOPT Integration**: ISRES, COBYLA, Nelder-Mead, and more
- **Differential Evolution**: Custom pure-Rust implementation
- **Particle Swarm**: Alternative global optimization
- **Hybrid Approaches**: Global + local refinement strategies

## Configuration

### Filter Parameters
```rust
use autoeq::{cli::Args, LossType};

let args = Args {
    num_filters: 5,           // Number of biquad sections
    max_db: 6.0,             // Maximum gain/cut in dB
    min_db: -12.0,           // Minimum gain/cut in dB
    max_q: 10.0,             // Maximum Q factor
    min_q: 0.1,              // Minimum Q factor
    min_freq: 20.0,          // Lowest filter frequency
    max_freq: 20000.0,       // Highest filter frequency
    min_spacing_oct: 0.25,   // Minimum filter spacing in octaves
    loss: LossType::Score,   // Optimization objective
    // ... other parameters
};
```

### Constraint System
- **Frequency Bounds**: Limit filters to audible range or specific bands
- **Gain Limits**: Prevent excessive boost/cut that could damage equipment
- **Q Factor Range**: Ensure filters are implementable and stable
- **Filter Spacing**: Avoid overlapping filters that waste parameters

## Data Integration

### Spinorama.org API
```rust
use autoeq::{read::fetch_measurement_plot_data, workflow};

// Fetch speaker data
let (curves, metadata) = fetch_measurement_plot_data(
    "KEF R3",
    "asr",
    "CEA2034"
).await?;

// Load and process for optimization
let (input_curve, spin_data) = workflow::load_input_curve(&args).await?;
```

### Local Files
Support for CSV, JSON, and other measurement formats with automatic format detection and interpolation.

## Output Formats

### PEQ Parameters
Standard parametric EQ format compatible with most audio software:
```
Type    Fc      Gain    Q
PK      85.0    -2.3    1.2
PK      240.0   1.8     0.8
LS      1200.0  -1.5    0.7
```

### Visualization
- Interactive Plotly charts showing before/after responses
- Filter visualization with frequency and phase response
- CEA2034 spinorama plots with EQ overlay
- Export to HTML, PNG, or SVG formats

## Performance

### Optimization Speed
- **Global Search**: ~10-50 iterations for convergence depending on algorithm
- **Local Refinement**: ~50-200 function evaluations for polish
- **Typical Runtime**: 2-30 seconds depending on complexity and population size

### Parallel Processing
- Multi-threaded optimization using thread pools
- Concurrent speaker processing in benchmark mode
- BLAS acceleration for linear algebra operations

## Integration

This crate integrates several AutoEQ ecosystem components:
- `autoeq-iir`: IIR filter implementations and response calculation
- `autoeq-de`: Differential Evolution global optimization
- `autoeq-cea2034`: Preference scoring and CEA2034 metrics

## Contributing

When adding features:
1. **Add tests** for new functionality
2. **Update documentation** and examples
3. **Validate against known results** where possible
4. **Consider backwards compatibility** for CLI arguments
5. **Update WARP.md** with new commands or workflows

## License

GPL-3.0-or-later
