# Multi-Driver Crossover Optimization Examples

This directory contains example driver measurement files for testing multi-driver crossover optimization.

## Files

- `woofer.csv` - Woofer frequency response (low frequency driver)
- `tweeter.csv` - Tweeter frequency response (high frequency driver)

## CSV Format

Each CSV file contains frequency response measurements with columns:
- `frequency` - Frequency in Hz
- `spl` - Sound pressure level in dB

Optionally, a third column `phase` can be included for phase measurements in degrees.

## Usage

### 2-Way Speaker Example

Optimize crossover for a 2-way speaker (woofer + tweeter):

```bash
cargo run --bin autoeq --release -- \
  --loss drivers-flat \
  --driver1 examples/drivers/woofer.csv \
  --driver2 examples/drivers/tweeter.csv \
  --crossover-type linkwitzriley4 \
  --algo nlopt:cobyla \
  --max-db 12.0
```

### Options

- `--loss drivers-flat` - Use multi-driver crossover optimization
- `--driver1`, `--driver2`, `--driver3`, `--driver4` - Paths to driver measurement CSV files (2-4 drivers supported)
- `--crossover-type` - Crossover filter type:
  - `butterworth2` - 2nd order Butterworth (12 dB/octave)
  - `linkwitzriley2` - 2nd order Linkwitz-Riley (12 dB/octave)
  - `linkwitzriley4` - 4th order Linkwitz-Riley (24 dB/octave, default)
- `--max-db` - Maximum gain adjustment per driver (default: 3.0 dB)
- `--min-freq`, `--max-freq` - Frequency range for optimization (default: 60-16000 Hz)
- `--algo` - Optimization algorithm (e.g., `nlopt:cobyla`, `nlopt:isres`, `mh:de`)

### Output

The optimization will output:
1. Driver gains in dB for each driver
2. Crossover frequencies between successive driver pairs
3. Crossover filter type used
4. Loss values (RMS deviation from flat response)

### Example Output

```
🎵 Multi-driver crossover optimization mode

✓ Loaded driver 1 from examples/drivers/woofer.csv
✓ Loaded driver 2 from examples/drivers/tweeter.csv
✓ Initialized 2 drivers with LinkwitzRiley4 crossover

📊 Drivers sorted by frequency (lowest to highest):
   Driver 1: 20 Hz - 20000 Hz (mean: 632 Hz)
   Driver 2: 20 Hz - 20000 Hz (mean: 632 Hz)

🎯 Optimization parameters:
   2 driver gains + 1 crossover frequencies = 3 parameters
   Gain bounds: [-12.0, 12.0] dB

🚀 Starting optimization...

✅ Optimization complete!

📊 Results:

Driver Gains:
   Driver 1: -2.50 dB
   Driver 2: +3.20 dB

Crossover Frequencies:
   Between Driver 1 and 2: 2500 Hz

Crossover Type: LinkwitzRiley4

Loss (RMS deviation from flat):
   Before optimization: 8.234567 dB
   After optimization:  1.234567 dB
   Improvement: 85.02%
```

## Creating Your Own Measurements

To use your own driver measurements:

1. Create a CSV file for each driver
2. Include at minimum `frequency` and `spl` columns
3. Optionally include `phase` column
4. Use consistent frequency points across all drivers (or let the tool interpolate)
5. Ensure drivers cover different frequency ranges (e.g., woofer for low frequencies, tweeter for high frequencies)

The tool will automatically:
- Sort drivers by their mean frequency (woofer → midrange → tweeter)
- Interpolate all measurements to a common frequency grid
- Determine reasonable crossover frequency bounds based on driver frequency ranges
