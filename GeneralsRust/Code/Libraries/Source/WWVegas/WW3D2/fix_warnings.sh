#!/bin/bash

# Track changes
CHANGES_LOG=""
FILES_MODIFIED=()
WARNINGS_FIXED=0
START_TIME=$(date +%s)

echo "Starting systematic cleanup of unused warnings..."
echo "=================================================="

# Fix 1: ww3d-core w3d_io.rs
echo "Fixing ww3d-core/src/w3d_io.rs..."
sed -i '' 's/fn read_emitter_user_data(&mut self, size: u32)/fn read_emitter_user_data(\&mut self, _size: u32)/' crates/ww3d-core/src/w3d_io.rs
sed -i '' '402d' crates/ww3d-core/src/w3d_io.rs
FILES_MODIFIED+=("crates/ww3d-core/src/w3d_io.rs")
WARNINGS_FIXED=$((WARNINGS_FIXED + 2))
CHANGES_LOG="${CHANGES_LOG}\n✓ ww3d-core: Prefixed 'size' with underscore, removed unused import"

# Fix 2: ww3d-scene scene_ext.rs - unused import Vec3
echo "Fixing ww3d-scene/src/scene_ext.rs..."
sed -i '' 's/use glam::{Vec2, Vec3};/use glam::Vec2;/' crates/ww3d-scene/src/scene_ext.rs
FILES_MODIFIED+=("crates/ww3d-scene/src/scene_ext.rs")
WARNINGS_FIXED=$((WARNINGS_FIXED + 1))
CHANGES_LOG="${CHANGES_LOG}\n✓ ww3d-scene: Removed unused Vec3 import"

# Fix 3: ww3d-scene lib.rs - unused variable pass_index
echo "Fixing ww3d-scene/src/lib.rs..."
sed -i '' 's/for pass_index in 0..max_passes {/for _pass_index in 0..max_passes {/' crates/ww3d-scene/src/lib.rs
FILES_MODIFIED+=("crates/ww3d-scene/src/lib.rs")
WARNINGS_FIXED=$((WARNINGS_FIXED + 1))
CHANGES_LOG="${CHANGES_LOG}\n✓ ww3d-scene: Prefixed 'pass_index' with underscore"

# Fix 4: ww3d-scene htree.rs - unused constant W3D_NAME_LEN
echo "Fixing ww3d-scene/src/htree.rs..."
sed -i '' 's/^const W3D_NAME_LEN: usize = 32;/#[allow(dead_code)]\nconst W3D_NAME_LEN: usize = 32;/' crates/ww3d-scene/src/htree.rs
FILES_MODIFIED+=("crates/ww3d-scene/src/htree.rs")
WARNINGS_FIXED=$((WARNINGS_FIXED + 1))
CHANGES_LOG="${CHANGES_LOG}\n✓ ww3d-scene: Allowed dead code for W3D_NAME_LEN constant"

echo "Script created - will run individual fixes next"
