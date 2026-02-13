#!/bin/bash
# Quick test script for phantom-simulation
# Validates code structure without full FHE compilation

echo "╔═══════════════════════════════════════════════════════════════╗"
echo "║    PHANTOM SIMULATION - QUICK STRUCTURE VALIDATION           ║"
echo "╚═══════════════════════════════════════════════════════════════╝"
echo ""

cd /run/media/zajferx/Data/dev/The-No-hands-Company/projects/Active/Phantom

echo "✓ Checking phantom-simulation crate structure..."
echo ""

echo "Files created:"
ls -lh crates/phantom-simulation/src/*.rs 2>/dev/null && echo "  ✓ Source files present" || echo "  ✗ Missing source files"
ls -lh crates/phantom-simulation/examples/*.rs 2>/dev/null && echo "  ✓ Examples present" || echo "  ✗ Missing examples"
ls -lh crates/phantom-simulation/benches/*.rs 2>/dev/null && echo "  ✓ Benchmarks present" || echo "  ✗ Missing benchmarks"
echo ""

echo "Code statistics:"
find crates/phantom-simulation/src -name "*.rs" -exec wc -l {} + | tail -1 | awk '{print "  Source lines: " $1}'
find crates/phantom-simulation/examples -name "*.rs" -exec wc -l {} + 2>/dev/null | tail -1 | awk '{print "  Example lines: " $1}'
find crates/phantom-simulation/benches -name "*.rs" -exec wc -l {} + 2>/dev/null | tail -1 | awk '{print "  Benchmark lines: " $1}'
echo ""

echo "Module structure:"
grep -h "^pub mod\|^pub use" crates/phantom-simulation/src/lib.rs | head -10
echo ""

echo "✓ Structure validation complete!"
echo ""
echo "Note: Full compilation requires ~5-10 minutes due to FHE dependencies."
echo "To build and run:"
echo "  cargo build --package phantom-simulation --release"
echo "  cargo run --package phantom-simulation --example network_simulation --release"
