#!/bin/bash

# List of files to convert
files=(
    "optde_ackley_n2.rs"
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

for file in "${files[@]}"; do
    echo "Processing $file..."
    
    # Update the use statement to include run_recorded_differential_evolution
    sed -i '' 's/use autoeq_de::{differential_evolution, /use autoeq_de::{run_recorded_differential_evolution, /' "$file"
    
    # If the file only imports differential_evolution
    sed -i '' 's/use autoeq_de::differential_evolution;/use autoeq_de::run_recorded_differential_evolution;/' "$file"
    
    echo "  Done with $file"
done

echo "All files processed!"
