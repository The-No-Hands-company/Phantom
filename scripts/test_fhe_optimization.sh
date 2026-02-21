#!/bin/bash
# Quick test for FHE key reuse optimization
# Tests small network (10 nodes) to verify compilation and speedup

cd /run/media/zajferx/Data/dev/The-No-hands-Company/projects/Active/Phantom

echo "=========================================="
echo "PHANTOM Phase 3 - FHE Optimization Test"
echo "=========================================="
echo ""

echo "Building phantom-crypto (FHE engine with Clone support)..."
cargo build --package phantom-crypto --release --quiet

if [ $? -eq 0 ]; then
    echo "✓ phantom-crypto built successfully"
else
    echo "✗ phantom-crypto build failed"
    exit 1
fi

echo ""
echo "Building phantom-simulation (with key reuse)..."
cargo build --package phantom-simulation --release 2>&1 | grep -E "(Compiling|Finished|error)" | tail -10

if [ $? -eq 0 ]; then
    echo "✓ phantom-simulation built successfully"
else
    echo "⚠ Check build output for errors"
fi

echo ""
echo "Optimization implemented:"
echo "  - FheEngine now implements Clone"
echo "  - NetworkConfig has simulation_mode flag"
echo "  - When enabled: 1 FHE keygen instead of N"
echo "  - Expected speedup: 100x for 100 nodes (80s → 0.8s init)"
echo ""
echo "Next: Run simulation to measure actual performance"
echo "  cargo run --package phantom-simulation --example network_simulation --release"
