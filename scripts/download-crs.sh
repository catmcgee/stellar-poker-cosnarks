#!/usr/bin/env bash
# Download BN254 CRS files required by co-noir for UltraHonk proof generation.
#
# Prerequisites:
#   cargo install --git https://github.com/TaceoLabs/co-snarks --branch main co-noir
#
# Usage:
#   ./scripts/download-crs.sh [output_dir]

set -euo pipefail

CRS_DIR="${1:-./crs}"

echo "=== Downloading BN254 CRS files ==="
echo "Output directory: ${CRS_DIR}"

mkdir -p "${CRS_DIR}"

# co-noir download-crs fetches the BN254 SRS points file
# --crs specifies the output file path, --num-points how many G1 points to download
co-noir download-crs --crs "${CRS_DIR}/bn254_g1.dat" --num-points 4194304

echo ""
echo "=== CRS files downloaded ==="
ls -lh "${CRS_DIR}"/*.dat 2>/dev/null || echo "Warning: no .dat files found in ${CRS_DIR}"
echo ""
echo "Done. CRS files are ready for MPC proof generation."
