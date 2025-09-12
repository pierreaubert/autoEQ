#!/bin/bash

# Files that still need conversion
FILES=(
    "optde_binh_korn_constrained.rs"
    "optde_cosine_mixture.rs"
    "optde_dejong_family.rs"
    "optde_dixons_price.rs"
    "optde_freudenstein_roth.rs"
    "optde_keanes_bump_constrained.rs"
    "optde_lampinen.rs"
    "optde_levi13.rs"
    "optde_mishras_bird_constrained.rs"
    "optde_powell.rs"
    "optde_quadratic.rs"
    "optde_quartic.rs"
    "optde_rosenbrock_constrained.rs"
    "optde_rotated_hyper_ellipsoid.rs"
    "optde_zakharov_nd.rs"
)

for file in "${FILES[@]}"; do
    echo "Converting $file..."
    
    # Step 1: Fix the use statement
    sed -i '' 's/use autoeq_de::{differential_evolution,/use autoeq_de::{run_recorded_differential_evolution,/' "$file"
    sed -i '' 's/use autoeq_de::differential_evolution;/use autoeq_de::run_recorded_differential_evolution;/' "$file"
    
    # Step 2: Add run_recorded_differential_evolution if not present
    if ! grep -q "run_recorded_differential_evolution" "$file"; then
        sed -i '' 's/use autoeq_de::{/use autoeq_de::{run_recorded_differential_evolution, /' "$file"
    fi
    
    echo "  ✓ Updated imports for $file"
done

echo ""
echo "All files have been updated with correct imports."
echo "Manual conversion of differential_evolution calls is still needed."
