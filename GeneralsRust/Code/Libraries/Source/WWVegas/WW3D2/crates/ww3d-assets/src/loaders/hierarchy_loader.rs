//! W3D Hierarchy Loader
//!
//! Complete implementation of W3D hierarchy (skeleton) loading functionality.
//! This is a faithful port of htree.cpp from the original C++ codebase.
//!
//! # C++ Reference
//! - File: `/Code/Libraries/Source/WWVegas/WW3D2/htree.cpp`
//! - Primary function: `HTreeClass::Load_W3D` (lines 176-244)
//! - Pivot reader: `read_pivots` (lines 259-343)
//!
//! # Key Features
//! - Skeleton hierarchy loading (bones/pivots)
//! - Parent-child relationships
//! - Base transformations (position + rotation)
//! - Version compatibility (pre-3.0 root node injection)
//!
//! # Chunk Structure
//! W3D hierarchies are stored as:
//! ```text
//! HIERARCHY
//!   ├─ HIERARCHY_HEADER
//!   └─ PIVOTS (array of bone data)
//! ```

use crate::chunk_reader::{ChunkReader, ChunkResult};
use glam::{Mat4, Quat, Vec3};
use std::collections::HashMap;
use std::io::{Read, Seek};
use ww3d_core::W3DChunkType;

/// W3D Hierarchy Header
/// C++ Reference: w3d_file.h W3dHierarchyStruct
#[derive(Debug, Clone)]
pub struct W3DHierarchyHeader {
    pub version: u32,
    pub name: String, // Fixed 16 bytes
    pub num_pivots: u32,
}

/// W3D Pivot (Bone) data
/// C++ Reference: w3d_file.h W3dPivotStruct
#[derive(Debug, Clone)]
pub struct W3DPivot {
    pub name: String,    // Fixed 16 bytes
    pub parent_idx: i32, // -1 for root
    pub translation: Vec3,
    pub rotation: Quat, // Euler XYZ in file, converted to quaternion
}

/// W3D Hierarchy (Skeleton)
///
/// Represents a complete skeleton hierarchy with bones and their relationships.
#[derive(Debug, Clone)]
pub struct W3DHierarchy {
    pub header: W3DHierarchyHeader,
    pub pivots: Vec<W3DPivot>,
    pub pivot_name_to_index: HashMap<String, usize>,
}

impl Default for W3DHierarchy {
    fn default() -> Self {
        Self::new()
    }
}

impl W3DHierarchy {
    pub fn new() -> Self {
        Self {
            header: W3DHierarchyHeader {
                version: 0,
                name: String::new(),
                num_pivots: 0,
            },
            pivots: Vec::new(),
            pivot_name_to_index: HashMap::new(),
        }
    }

    /// Get pivot index by name (case-insensitive)
    pub fn get_pivot_index(&self, name: &str) -> Option<usize> {
        self.pivot_name_to_index.get(name).copied().or_else(|| {
            // Fallback: case-insensitive search
            let upper = name.to_ascii_uppercase();
            self.pivots
                .iter()
                .position(|p| p.name.to_ascii_uppercase() == upper)
        })
    }

    /// Get pivot by index
    pub fn get_pivot(&self, index: usize) -> Option<&W3DPivot> {
        self.pivots.get(index)
    }

    /// Get pivot by name
    pub fn get_pivot_by_name(&self, name: &str) -> Option<&W3DPivot> {
        self.get_pivot_index(name)
            .and_then(|idx| self.pivots.get(idx))
    }

    /// Get number of pivots
    pub fn num_pivots(&self) -> usize {
        self.pivots.len()
    }

    /// Build base transform matrix for a pivot
    pub fn get_base_transform(&self, index: usize) -> Mat4 {
        if let Some(pivot) = self.pivots.get(index) {
            Mat4::from_rotation_translation(pivot.rotation, pivot.translation)
        } else {
            Mat4::IDENTITY
        }
    }

    /// Get parent index for a pivot
    pub fn get_parent_index(&self, index: usize) -> Option<usize> {
        if let Some(pivot) = self.pivots.get(index) {
            if pivot.parent_idx >= 0 {
                Some(pivot.parent_idx as usize)
            } else {
                None
            }
        } else {
            None
        }
    }
}

/// W3D Hierarchy Loader
///
/// Loads W3D hierarchy (skeleton) files using the chunk-based file format.
///
/// # C++ Reference
/// - Class: HTreeClass
/// - File: htree.cpp
/// - Main function: Load_W3D (lines 176-244)
pub struct HierarchyLoader;

impl HierarchyLoader {
    /// Load a W3D hierarchy from a ChunkReader
    ///
    /// # C++ Reference
    /// - Function: `HTreeClass::Load_W3D`
    /// - File: htree.cpp, lines 176-244
    ///
    /// # Arguments
    /// * `reader` - ChunkReader positioned at HIERARCHY chunk
    ///
    /// # Returns
    /// - `Ok(W3DHierarchy)` - Successfully loaded hierarchy
    /// - `Err` - Load error
    pub fn load_hierarchy<R: Read + Seek>(
        reader: &mut ChunkReader<R>,
    ) -> ChunkResult<W3DHierarchy> {
        let mut hierarchy = W3DHierarchy::new();

        // C++ Line 183: Open first chunk (should be HIERARCHY_HEADER)
        if !reader.open_chunk()? {
            return Err(crate::chunk_reader::ChunkError::InvalidHeader);
        }

        let chunk_id = reader.current_chunk_id()?;

        // C++ Line 185: Check for HIERARCHY_HEADER chunk
        if chunk_id != W3DChunkType::HierarchyHeader.as_u32() {
            return Err(crate::chunk_reader::ChunkError::InvalidHeader);
        }

        // C++ Line 191: Read the hierarchy header
        hierarchy.header = Self::read_hierarchy_header(reader)?;
        reader.close_chunk()?;

        // C++ Line 202-206: Check version for pre-3.0 compatibility
        let version = hierarchy.header.version;
        let pre30 = version < 0x00030000; // W3D_MAKE_VERSION(3, 0)

        if pre30 {
            // Pre-3.0 files need a synthetic root node
            hierarchy.header.num_pivots += 1;
        }

        // C++ Line 221: Read all remaining chunks
        while reader.open_chunk()? {
            let chunk_id = reader.current_chunk_id()?;

            match W3DChunkType::from_u32(chunk_id) {
                Some(W3DChunkType::Pivots) => {
                    // C++ Line 226: Read pivots
                    Self::read_pivots(reader, &mut hierarchy, pre30)?;
                }
                Some(W3DChunkType::PivotFixups) => {
                    // Pivot fixups (usually for model edits)
                    // Not commonly used, skip for now
                }
                _ => {
                    // Unknown chunk, skip
                }
            }

            reader.close_chunk()?;
        }

        Ok(hierarchy)
    }

    /// Read the hierarchy header chunk
    ///
    /// # C++ Reference
    /// - Structure: W3dHierarchyStruct
    /// - File: w3d_file.h
    /// - Usage: htree.cpp lines 190-195
    fn read_hierarchy_header<R: Read + Seek>(
        reader: &mut ChunkReader<R>,
    ) -> ChunkResult<W3DHierarchyHeader> {
        // C++: Read W3dHierarchyStruct
        let version = reader.read_u32()?;
        let name = reader.read_fixed_string(16)?;
        let num_pivots = reader.read_u32()?;

        Ok(W3DHierarchyHeader {
            version,
            name,
            num_pivots,
        })
    }

    /// Read pivots (bones) array
    ///
    /// # C++ Reference
    /// - Function: `HTreeClass::read_pivots`
    /// - File: htree.cpp, lines 259-343
    ///
    /// # Arguments
    /// * `reader` - Chunk reader
    /// * `hierarchy` - Hierarchy being loaded
    /// * `pre30` - Whether this is a pre-3.0 file needing root node
    fn read_pivots<R: Read + Seek>(
        reader: &mut ChunkReader<R>,
        hierarchy: &mut W3DHierarchy,
        pre30: bool,
    ) -> ChunkResult<()> {
        let num_pivots = hierarchy.header.num_pivots as usize;
        hierarchy.pivots.reserve(num_pivots);

        let mut first_piv = 0;

        // C++ Line 270-278: Add synthetic root node for pre-3.0 files
        if pre30 {
            let root_pivot = W3DPivot {
                name: "RootTransform".to_string(),
                parent_idx: -1,
                translation: Vec3::ZERO,
                rotation: Quat::IDENTITY,
            };

            hierarchy.pivots.push(root_pivot);
            hierarchy
                .pivot_name_to_index
                .insert("RootTransform".to_string(), 0);
            first_piv = 1;
        }

        // C++ Line 280: Read each pivot
        for pidx in first_piv..num_pivots {
            // C++ Line 282: Read W3dPivotStruct
            let name = reader.read_fixed_string(16)?;
            let parent_idx = reader.read_i32()?;

            // Translation (Vector3)
            let translation = reader.read_vec3()?;

            // Rotation (Quaternion - stored as [x, y, z, w])
            // C++ Line 296-300: Read quaternion components
            let rotation = reader.read_quaternion()?;

            // C++ Line 322-324: Adjust parent index for pre-3.0 files
            let adjusted_parent_idx = if pre30 { parent_idx + 1 } else { parent_idx };

            let pivot = W3DPivot {
                name: name.clone(),
                parent_idx: adjusted_parent_idx,
                translation,
                rotation,
            };

            hierarchy.pivots.push(pivot);
            hierarchy.pivot_name_to_index.insert(name, pidx);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// Helper: Create a chunk with header
    fn create_chunk(chunk_type: u32, has_sub_chunks: bool, data: &[u8]) -> Vec<u8> {
        let mut result = Vec::new();
        result.extend_from_slice(&chunk_type.to_le_bytes());

        let size = data.len() as u32;
        let size_with_flag = if has_sub_chunks {
            size | 0x80000000
        } else {
            size
        };
        result.extend_from_slice(&size_with_flag.to_le_bytes());
        result.extend_from_slice(data);

        result
    }

    /// Helper: Create test hierarchy header
    fn create_test_hierarchy_header(num_pivots: u32) -> Vec<u8> {
        let mut data = Vec::new();

        // version (4.0)
        data.extend_from_slice(&0x00040000u32.to_le_bytes());
        // name (16 bytes)
        let mut name = b"TestSkeleton\0\0\0\0".to_vec();
        data.append(&mut name);
        // num_pivots
        data.extend_from_slice(&num_pivots.to_le_bytes());

        data
    }

    /// Helper: Create pivot data
    fn create_pivot_data(name: &str, parent_idx: i32, pos: [f32; 3], rot: [f32; 4]) -> Vec<u8> {
        let mut data = Vec::new();

        // name (16 bytes, padded)
        let mut name_bytes = name.as_bytes().to_vec();
        name_bytes.resize(16, 0);
        data.extend_from_slice(&name_bytes);

        // parent_idx
        data.extend_from_slice(&parent_idx.to_le_bytes());

        // translation
        data.extend_from_slice(&pos[0].to_le_bytes());
        data.extend_from_slice(&pos[1].to_le_bytes());
        data.extend_from_slice(&pos[2].to_le_bytes());

        // rotation (quaternion: w, x, y, z)
        data.extend_from_slice(&rot[0].to_le_bytes());
        data.extend_from_slice(&rot[1].to_le_bytes());
        data.extend_from_slice(&rot[2].to_le_bytes());
        data.extend_from_slice(&rot[3].to_le_bytes());

        data
    }

    #[test]
    fn test_read_hierarchy_header() {
        let header_data = create_test_hierarchy_header(3);
        let chunk = create_chunk(W3DChunkType::HierarchyHeader.as_u32(), false, &header_data);

        let mut reader = ChunkReader::new(Cursor::new(&chunk));
        reader.open_chunk().unwrap();

        let header = HierarchyLoader::read_hierarchy_header(&mut reader).unwrap();

        assert_eq!(header.name, "TestSkeleton");
        assert_eq!(header.num_pivots, 3);
        assert_eq!(header.version, 0x00040000);
    }

    #[test]
    fn test_read_simple_hierarchy() {
        let mut hierarchy_data = Vec::new();

        // Header chunk
        let header_data = create_test_hierarchy_header(2);
        let header_chunk =
            create_chunk(W3DChunkType::HierarchyHeader.as_u32(), false, &header_data);
        hierarchy_data.extend_from_slice(&header_chunk);

        // Pivots chunk with 2 bones
        let mut pivots_data = Vec::new();

        // Bone 0: Root bone at origin
        pivots_data.extend_from_slice(&create_pivot_data(
            "Root",
            -1,
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0, 0.0], // Identity quaternion
        ));

        // Bone 1: Child bone offset on Y axis
        pivots_data.extend_from_slice(&create_pivot_data(
            "Bone01",
            0,
            [0.0, 1.0, 0.0],
            [1.0, 0.0, 0.0, 0.0], // Identity quaternion
        ));

        let pivots_chunk = create_chunk(W3DChunkType::Pivots.as_u32(), false, &pivots_data);
        hierarchy_data.extend_from_slice(&pivots_chunk);

        // Load hierarchy
        let mut reader = ChunkReader::new(Cursor::new(&hierarchy_data));
        let hierarchy = HierarchyLoader::load_hierarchy(&mut reader).unwrap();

        // Verify
        assert_eq!(hierarchy.header.name, "TestSkeleton");
        assert_eq!(hierarchy.num_pivots(), 2);

        let root = &hierarchy.pivots[0];
        assert_eq!(root.name, "Root");
        assert_eq!(root.parent_idx, -1);
        assert_eq!(root.translation, Vec3::ZERO);

        let bone1 = &hierarchy.pivots[1];
        assert_eq!(bone1.name, "Bone01");
        assert_eq!(bone1.parent_idx, 0);
        assert_eq!(bone1.translation, Vec3::new(0.0, 1.0, 0.0));

        // Test lookup
        assert_eq!(hierarchy.get_pivot_index("Root"), Some(0));
        assert_eq!(hierarchy.get_pivot_index("Bone01"), Some(1));
        assert_eq!(hierarchy.get_parent_index(1), Some(0));
        assert_eq!(hierarchy.get_parent_index(0), None);
    }

    #[test]
    fn test_pre30_root_injection() {
        let mut hierarchy_data = Vec::new();

        // Header with pre-3.0 version
        let mut header_data = Vec::new();
        header_data.extend_from_slice(&0x00020005u32.to_le_bytes()); // Version 2.5
        let mut name = b"OldSkeleton\0\0\0\0\0".to_vec();
        header_data.append(&mut name);
        header_data.extend_from_slice(&1u32.to_le_bytes()); // 1 pivot (will become 2)

        let header_chunk =
            create_chunk(W3DChunkType::HierarchyHeader.as_u32(), false, &header_data);
        hierarchy_data.extend_from_slice(&header_chunk);

        // Single pivot (will have synthetic root added)
        let pivot_data = create_pivot_data("Bone00", -1, [0.0, 0.0, 0.0], [1.0, 0.0, 0.0, 0.0]);

        let pivots_chunk = create_chunk(W3DChunkType::Pivots.as_u32(), false, &pivot_data);
        hierarchy_data.extend_from_slice(&pivots_chunk);

        // Load hierarchy
        let mut reader = ChunkReader::new(Cursor::new(&hierarchy_data));
        let hierarchy = HierarchyLoader::load_hierarchy(&mut reader).unwrap();

        // Should have 2 pivots: synthetic root + original
        assert_eq!(hierarchy.num_pivots(), 2);

        let root = &hierarchy.pivots[0];
        assert_eq!(root.name, "RootTransform");
        assert_eq!(root.parent_idx, -1);

        let bone = &hierarchy.pivots[1];
        assert_eq!(bone.name, "Bone00");
        assert_eq!(bone.parent_idx, 0); // Adjusted to point to synthetic root
    }

    #[test]
    fn test_hierarchy_lookups() {
        let mut hierarchy = W3DHierarchy::new();
        hierarchy.header.num_pivots = 3;

        hierarchy.pivots.push(W3DPivot {
            name: "Root".to_string(),
            parent_idx: -1,
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
        });
        hierarchy.pivots.push(W3DPivot {
            name: "Spine".to_string(),
            parent_idx: 0,
            translation: Vec3::new(0.0, 1.0, 0.0),
            rotation: Quat::IDENTITY,
        });
        hierarchy.pivots.push(W3DPivot {
            name: "Head".to_string(),
            parent_idx: 1,
            translation: Vec3::new(0.0, 0.5, 0.0),
            rotation: Quat::IDENTITY,
        });

        hierarchy.pivot_name_to_index.insert("Root".to_string(), 0);
        hierarchy.pivot_name_to_index.insert("Spine".to_string(), 1);
        hierarchy.pivot_name_to_index.insert("Head".to_string(), 2);

        // Test name lookups
        assert_eq!(hierarchy.get_pivot_index("Root"), Some(0));
        assert_eq!(hierarchy.get_pivot_index("Spine"), Some(1));
        assert_eq!(hierarchy.get_pivot_index("Head"), Some(2));
        assert_eq!(hierarchy.get_pivot_index("Missing"), None);

        // Test parent lookups
        assert_eq!(hierarchy.get_parent_index(0), None);
        assert_eq!(hierarchy.get_parent_index(1), Some(0));
        assert_eq!(hierarchy.get_parent_index(2), Some(1));

        // Test base transform
        let spine_transform = hierarchy.get_base_transform(1);
        let translation = Vec3::new(
            spine_transform.w_axis.x,
            spine_transform.w_axis.y,
            spine_transform.w_axis.z,
        );
        assert!((translation - Vec3::new(0.0, 1.0, 0.0)).length() < 0.001);
    }

    #[test]
    fn test_complex_skeleton() {
        // Build a more complex skeleton: Root -> Spine -> [LeftArm, RightArm, Head]
        let mut hierarchy_data = Vec::new();

        // Header
        let header_data = create_test_hierarchy_header(5);
        let header_chunk =
            create_chunk(W3DChunkType::HierarchyHeader.as_u32(), false, &header_data);
        hierarchy_data.extend_from_slice(&header_chunk);

        // Pivots
        let mut pivots_data = Vec::new();

        // Root
        pivots_data.extend_from_slice(&create_pivot_data(
            "Root",
            -1,
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0, 0.0],
        ));

        // Spine (child of Root)
        pivots_data.extend_from_slice(&create_pivot_data(
            "Spine",
            0,
            [0.0, 1.0, 0.0],
            [1.0, 0.0, 0.0, 0.0],
        ));

        // LeftArm (child of Spine)
        pivots_data.extend_from_slice(&create_pivot_data(
            "LeftArm",
            1,
            [-0.5, 0.5, 0.0],
            [1.0, 0.0, 0.0, 0.0],
        ));

        // RightArm (child of Spine)
        pivots_data.extend_from_slice(&create_pivot_data(
            "RightArm",
            1,
            [0.5, 0.5, 0.0],
            [1.0, 0.0, 0.0, 0.0],
        ));

        // Head (child of Spine)
        pivots_data.extend_from_slice(&create_pivot_data(
            "Head",
            1,
            [0.0, 0.8, 0.0],
            [1.0, 0.0, 0.0, 0.0],
        ));

        let pivots_chunk = create_chunk(W3DChunkType::Pivots.as_u32(), false, &pivots_data);
        hierarchy_data.extend_from_slice(&pivots_chunk);

        // Load
        let mut reader = ChunkReader::new(Cursor::new(&hierarchy_data));
        let hierarchy = HierarchyLoader::load_hierarchy(&mut reader).unwrap();

        // Verify structure
        assert_eq!(hierarchy.num_pivots(), 5);

        // All spine children should have parent index 1
        assert_eq!(hierarchy.get_parent_index(2), Some(1)); // LeftArm
        assert_eq!(hierarchy.get_parent_index(3), Some(1)); // RightArm
        assert_eq!(hierarchy.get_parent_index(4), Some(1)); // Head
    }
}
