//! Mesh Data Structures
//!
//! This module provides mesh data structures for shader rendering.

use crate::error::ShdResult;
use crate::interface::VertexStreams;

/// Shader mesh structure
#[derive(Debug)]
pub struct ShdMesh {
    /// Vertex data streams
    pub vertex_streams: VertexStreams,

    /// Index buffer (if present)
    pub indices: Option<Vec<u32>>,

    /// Mesh name for debugging
    pub name: String,
}

impl ShdMesh {
    /// Create a new shader mesh
    pub fn new(name: String) -> Self {
        Self {
            vertex_streams: VertexStreams::new(),
            indices: None,
            name,
        }
    }

    /// Get the number of vertices in this mesh
    pub fn vertex_count(&self) -> usize {
        self.vertex_streams.get_vertex_count()
    }

    /// Get the number of triangles in this mesh
    pub fn triangle_count(&self) -> usize {
        if let Some(indices) = &self.indices {
            indices.len() / 3
        } else {
            self.vertex_count() / 3
        }
    }

    /// Validate the mesh data
    pub fn validate(&self) -> ShdResult<()> {
        if self.vertex_count() == 0 {
            return Err(crate::error::ShdError::InvalidConfig(
                "Mesh has no vertices".to_string(),
            ));
        }

        if let Some(indices) = &self.indices {
            let max_index = self.vertex_count() as u32;
            for &index in indices {
                if index >= max_index {
                    return Err(crate::error::ShdError::InvalidConfig(format!(
                        "Index {} is out of range for {} vertices",
                        index, max_index
                    )));
                }
            }
        }

        Ok(())
    }
}

impl Default for ShdMesh {
    fn default() -> Self {
        Self::new("Unnamed Mesh".to_string())
    }
}
