#!/bin/bash
set -e

# Cross-compilation build script for AutoEQ
# Builds CLI binaries for macOS, Linux, and Windows

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Ensure we're in the project root
if [ ! -f Cargo.toml ]; then
    echo -e "${RED}Error: Please run this script from the project root directory${NC}"
    exit 1
fi

# Add cargo to PATH
export PATH="$HOME/.cargo/bin:$PATH"

echo -e "${GREEN}=== AutoEQ Cross-Platform Build Script ===${NC}"
echo

# Define targets as arrays
TARGETS=("aarch64-apple-darwin" "x86_64-apple-darwin" "x86_64-unknown-linux-gnu" "aarch64-unknown-linux-gnu" "x86_64-pc-windows-gnu")
DESCRIPTIONS=("macOS ARM64 (Apple Silicon)" "macOS Intel" "Linux x86_64" "Linux ARM64" "Windows x86_64")

# Function to get description for target
get_description() {
    local target=$1
    for i in "${!TARGETS[@]}"; do
        if [ "${TARGETS[$i]}" = "$target" ]; then
            echo "${DESCRIPTIONS[$i]}"
            return
        fi
    done
    echo "Unknown"
}

# Define binaries to build
BINARIES=("autoeq" "download" "benchmark")

# Create output directory
OUTPUT_DIR="dist"
rm -rf "$OUTPUT_DIR"
mkdir -p "$OUTPUT_DIR"

echo -e "${YELLOW}Building for the following platforms:${NC}"
for target in "${TARGETS[@]}"; do
    echo "  - $target ($(get_description "$target"))"
done
echo

# Function to build for a target
build_target() {
    local target=$1
    local description=$(get_description "$target")
    local success=true
    
    echo -e "${YELLOW}Building for $target ($description)...${NC}"
    
    # Create target directory
    mkdir -p "$OUTPUT_DIR/$target"
    
    for binary in "${BINARIES[@]}"; do
        echo -n "  - Building $binary... "
        
        # Choose build method based on target
        if [[ "$target" == *"apple-darwin" ]]; then
            # Use regular cargo for macOS targets
            if cargo build --release --target "$target" --bin "$binary" > /dev/null 2>&1; then
                echo -e "${GREEN}✓${NC}"
                cp "target/$target/release/$binary" "$OUTPUT_DIR/$target/"
            else
                echo -e "${RED}✗${NC}"
                success=false
            fi
        elif [[ "$target" == *"linux"* ]] || [[ "$target" == *"windows"* ]]; then
            # Use cross for Linux and Windows targets (requires Docker)
            if command -v cross >/dev/null 2>&1 && docker info >/dev/null 2>&1; then
                if cross build --release --target "$target" --bin "$binary" > /dev/null 2>&1; then
                    echo -e "${GREEN}✓${NC}"
                    # Copy binary with appropriate extension
                    if [[ "$target" == *"windows"* ]]; then
                        cp "target/$target/release/$binary.exe" "$OUTPUT_DIR/$target/"
                    else
                        cp "target/$target/release/$binary" "$OUTPUT_DIR/$target/"
                    fi
                else
                    echo -e "${RED}✗ (cross compilation failed)${NC}"
                    success=false
                fi
            else
                if command -v cross >/dev/null 2>&1; then
                    echo -e "${YELLOW}⚠ (Docker not available)${NC}"
                else
                    echo -e "${YELLOW}⚠ (cross not installed)${NC}"
                fi
                success=false
            fi
        fi
    done
    
    if [ "$success" = true ]; then
        echo -e "  ${GREEN}All binaries built successfully for $target${NC}"
        # Create a simple info file
        echo "AutoEQ binaries for $target ($description)" > "$OUTPUT_DIR/$target/README.txt"
        echo "Built on: $(date)" >> "$OUTPUT_DIR/$target/README.txt"
        echo "Binaries included:" >> "$OUTPUT_DIR/$target/README.txt"
        for binary in "${BINARIES[@]}"; do
            if [[ "$target" == *"windows"* ]]; then
                echo "  - $binary.exe" >> "$OUTPUT_DIR/$target/README.txt"
            else
                echo "  - $binary" >> "$OUTPUT_DIR/$target/README.txt"
            fi
        done
    else
        echo -e "  ${YELLOW}Some binaries failed to build for $target${NC}"
    fi
    echo
}

# Build native target first (current platform)
echo -e "${GREEN}Building for native platform first...${NC}"
cargo build --release
echo -e "${GREEN}Native build completed successfully${NC}"
echo

# Copy native binaries
NATIVE_ARCH=$(rustc --version --verbose | grep host | cut -d' ' -f2)
mkdir -p "$OUTPUT_DIR/$NATIVE_ARCH"
for binary in "${BINARIES[@]}"; do
    cp "target/release/$binary" "$OUTPUT_DIR/$NATIVE_ARCH/"
done
echo "AutoEQ binaries for $NATIVE_ARCH (native)" > "$OUTPUT_DIR/$NATIVE_ARCH/README.txt"
echo "Built on: $(date)" >> "$OUTPUT_DIR/$NATIVE_ARCH/README.txt"

# Build for all other targets
for target in "${TARGETS[@]}"; do
    if [ "$target" != "$NATIVE_ARCH" ]; then
        build_target "$target"
    fi
done

echo -e "${GREEN}=== Build Summary ===${NC}"
echo -e "Output directory: ${YELLOW}$OUTPUT_DIR/${NC}"
echo
echo "Available builds:"
for dir in "$OUTPUT_DIR"/*; do
    if [ -d "$dir" ]; then
        target=$(basename "$dir")
        count=$(find "$dir" -type f -executable -o -name "*.exe" | wc -l | tr -d ' ')
        echo "  - $target: $count binaries"
    fi
done

echo
echo -e "${GREEN}Build completed!${NC}"
echo
echo -e "${YELLOW}Next steps:${NC}"
echo "1. Test the binaries on their respective platforms"
echo "2. Create distribution packages/archives"
echo "3. Consider setting up GitHub Actions for automated builds"

# Optional: Create archives
if command -v tar >/dev/null 2>&1; then
    echo
    echo -e "${YELLOW}Creating distribution archives...${NC}"
    cd "$OUTPUT_DIR"
    for dir in */; do
        target=$(basename "$dir")
        if [ -d "$target" ]; then
            tar -czf "autoeq-$target.tar.gz" "$target/"
            echo "  - autoeq-$target.tar.gz"
        fi
    done
    cd ..
    echo -e "${GREEN}Archives created in $OUTPUT_DIR/${NC}"
fi
