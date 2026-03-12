#!/bin/bash
# Simple test to check texture availability

echo "=== Checking for texture files in archives ==="
find . -name "*.big" -type f 2>/dev/null | while read bigfile; do
    echo "Archive: $bigfile"
done

echo ""
echo "=== Checking what textures are loaded ==="
timeout 20 ./target/debug/generals 2>&1 | grep -i "texture" | head -30
