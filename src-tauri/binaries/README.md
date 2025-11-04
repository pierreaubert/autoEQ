# CamillaDSP Bundled Binaries

This directory contains CamillaDSP binaries that are bundled with the AutoEQ application.

## Current Binaries

- **camilladsp-aarch64-apple-darwin/**: ARM64 macOS (Apple Silicon)
  - Version: 3.0.1
  - Size: ~6.1 MB
  - Compiled from source locally

## Building

The binaries in this directory were compiled from the official CamillaDSP v3.0.1 release.

To rebuild for the current platform:

```bash
# Clone CamillaDSP
cd /tmp
git clone --depth 1 --branch v3.0.1 https://github.com/HEnquist/camilladsp.git camilladsp-build

# Build in release mode
cd camilladsp-build
cargo build --release

# Get current platform target
PLATFORM=$(rustc -vV | grep host | cut -d' ' -f2)

# Copy to binaries directory
mkdir -p /path/to/autoeq-app/src-ui/src-tauri/binaries/camilladsp-$PLATFORM
cp target/release/camilladsp /path/to/autoeq-app/src-ui/src-tauri/binaries/camilladsp-$PLATFORM/
```

## Platform Support

### Currently Bundled
- macOS ARM64 (aarch64-apple-darwin)
-️ Linux x86_64 (x86_64-unknown-linux-gnu)

### To Add
-️ macOS Intel (x86_64-apple-darwin)
-️ Windows x86_64 (x86_64-pc-windows-msvc)

## Tauri Configuration

The binaries are configured in `tauri.conf.json`:

```json
{
  "bundle": {
    "externalBin": [
      "binaries/camilladsp"
    ]
  }
}
```

Tauri automatically selects the correct binary for the target platform during build.

## Binary Detection

The application detects CamillaDSP binaries in this order:

1. **Bundled binary** (next to executable in production)
2. **System PATH** (e.g., from Homebrew installation)
3. **Common locations** (`/usr/local/bin`, `/opt/homebrew/bin`)

See `src-backend/src/camilla.rs::find_camilladsp_binary()` for implementation details.

## Licensing

CamillaDSP is licensed under GPL-3.0. See `CAMILLADSP_LICENSE.txt` in the project root for full licensing information.

**Key Points**:
- CamillaDSP runs as a separate subprocess
- No GPL code is embedded in AutoEQ
- Source code attribution provided
- Binaries are unmodified from official releases

## Size Optimization

To reduce binary size, consider:

1. **Strip symbols** (already done by cargo release build):
```bash
strip camilladsp
```

2. **Compress** (optional, may affect code signing):
```bash
upx --best camilladsp
```

Note: macOS binaries must be code-signed after any modifications.

## Verification

To verify a binary works:

```bash
./camilladsp-aarch64-apple-darwin/camilladsp --version
# Should output: CamillaDSP 3.0.1
```

## Adding New Platform Binaries

1. Obtain or compile CamillaDSP for the target platform
2. Create directory: `binaries/camilladsp-<target-triple>/`
3. Copy binary into directory
4. Test with: `cargo run --release --bin audio_test -- devices`
5. Commit binary to repository

## Notes

- Binaries are committed to git (git LFS not required for 6MB files)
- Each platform binary is ~6-8 MB
- Total size for all platforms: ~25-35 MB
- This is acceptable for desktop distribution

## References

- [CamillaDSP GitHub](https://github.com/HEnquist/camilladsp)
- [Tauri Sidecar Documentation](https://tauri.app/v1/guides/building/sidecar)
- [Binary Bundling Guide](../../../docs/CAMILLADSP_BUNDLING.md)
