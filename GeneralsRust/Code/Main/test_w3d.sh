#!/bin/bash
echo "Testing W3D model loading..."
RUST_LOG=error timeout 60s cargo run --bin generals 2>&1 | \
  grep -E "(W3D.*LOADING|FILE.*FOUND|FILE.*NOT FOUND|EXACT MATCH|ARCHIVE MATCH)" | \
  head -30
echo "Test completed."