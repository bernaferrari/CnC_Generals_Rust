/// W3D File I/O for AABTree serialization/deserialization
///
/// Implements loading and saving of AABTrees from/to W3D chunk format
/// Compatible with C++ WW3D2 chunk format
use crate::aabtree::{AABTree, CullNode};
use glam::Vec3;
use std::io::{self, Read, Write};

// W3D Chunk IDs (from w3d_file.h)
pub const W3D_CHUNK_AABTREE: u32 = 0x00000090;
pub const W3D_CHUNK_AABTREE_HEADER: u32 = 0x00000091;
pub const W3D_CHUNK_AABTREE_POLYINDICES: u32 = 0x00000092;
pub const W3D_CHUNK_AABTREE_NODES: u32 = 0x00000093;

/// W3D AABTree Header structure (matches C++ layout)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct W3dMeshAABTreeHeader {
    pub node_count: u32,
    pub poly_count: u32,
    pub padding: [u32; 6], // Reserved for future use
}

impl W3dMeshAABTreeHeader {
    pub fn new(node_count: u32, poly_count: u32) -> Self {
        Self {
            node_count,
            poly_count,
            padding: [0; 6],
        }
    }
}

/// W3D AABTree Node structure (matches C++ layout)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct W3dMeshAABTreeNode {
    pub min: [f32; 3],
    pub max: [f32; 3],
    pub front_or_poly0: u32,
    pub back_or_poly_count: u32,
}

impl W3dMeshAABTreeNode {
    pub fn from_cull_node(node: &CullNode) -> Self {
        Self {
            min: [node.min.x, node.min.y, node.min.z],
            max: [node.max.x, node.max.y, node.max.z],
            front_or_poly0: node.front_or_poly0,
            back_or_poly_count: node.back_or_poly_count,
        }
    }

    pub fn to_cull_node(&self) -> CullNode {
        CullNode {
            min: Vec3::new(self.min[0], self.min[1], self.min[2]),
            max: Vec3::new(self.max[0], self.max[1], self.max[2]),
            front_or_poly0: self.front_or_poly0,
            back_or_poly_count: self.back_or_poly_count,
        }
    }
}

/// W3D Chunk Header
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct W3dChunkHeader {
    pub chunk_type: u32,
    pub chunk_size: u32, // Size of chunk data (not including header)
}

impl W3dChunkHeader {
    pub fn new(chunk_type: u32, chunk_size: u32) -> Self {
        Self {
            chunk_type,
            chunk_size,
        }
    }

    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let mut buf = [0u8; 8];
        reader.read_exact(&mut buf)?;

        Ok(Self {
            chunk_type: u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]),
            chunk_size: u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]),
        })
    }

    pub fn write<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_all(&self.chunk_type.to_le_bytes())?;
        writer.write_all(&self.chunk_size.to_le_bytes())?;
        Ok(())
    }
}

/// AABTree W3D Loader/Saver
pub struct AABTreeIO;

impl AABTreeIO {
    /// Load AABTree from W3D chunk format
    pub fn load_w3d<R: Read>(reader: &mut R) -> io::Result<AABTree> {
        // Read main AABTREE chunk header
        let main_header = W3dChunkHeader::read(reader)?;
        if main_header.chunk_type != W3D_CHUNK_AABTREE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Not an AABTREE chunk",
            ));
        }

        let mut tree = AABTree::new();
        let mut header_read = false;
        let mut poly_indices_read = false;
        let mut nodes_read = false;

        let mut bytes_read = 0u32;
        let total_size = main_header.chunk_size;

        // Read sub-chunks
        while bytes_read < total_size {
            let sub_header = W3dChunkHeader::read(reader)?;
            bytes_read += 8; // Header size

            match sub_header.chunk_type {
                W3D_CHUNK_AABTREE_HEADER => {
                    let header = Self::read_header(reader)?;
                    tree.node_count = header.node_count as usize;
                    tree.poly_count = header.poly_count as usize;
                    tree.nodes = vec![CullNode::new(); tree.node_count];
                    tree.poly_indices = vec![0; tree.poly_count];
                    header_read = true;
                    bytes_read += sub_header.chunk_size;
                }
                W3D_CHUNK_AABTREE_POLYINDICES => {
                    if !header_read {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "Header must be read before poly indices",
                        ));
                    }
                    Self::read_poly_indices(reader, &mut tree.poly_indices)?;
                    poly_indices_read = true;
                    bytes_read += sub_header.chunk_size;
                }
                W3D_CHUNK_AABTREE_NODES => {
                    if !header_read {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "Header must be read before nodes",
                        ));
                    }
                    Self::read_nodes(reader, &mut tree.nodes)?;
                    nodes_read = true;
                    bytes_read += sub_header.chunk_size;
                }
                _ => {
                    // Skip unknown chunks
                    let mut skip_buf = vec![0u8; sub_header.chunk_size as usize];
                    reader.read_exact(&mut skip_buf)?;
                    bytes_read += sub_header.chunk_size;
                }
            }
        }

        if !header_read || !poly_indices_read || !nodes_read {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Incomplete AABTREE data",
            ));
        }

        Ok(tree)
    }

    /// Save AABTree to W3D chunk format
    pub fn save_w3d<W: Write>(tree: &AABTree, writer: &mut W) -> io::Result<()> {
        // Calculate total size
        let header_size = std::mem::size_of::<W3dMeshAABTreeHeader>() as u32;
        let poly_indices_size = (tree.poly_count * std::mem::size_of::<u32>()) as u32;
        let nodes_size = (tree.node_count * std::mem::size_of::<W3dMeshAABTreeNode>()) as u32;

        // 3 sub-chunk headers + data
        let total_size = 8 + header_size + 8 + poly_indices_size + 8 + nodes_size;

        // Write main chunk header
        let main_header = W3dChunkHeader::new(W3D_CHUNK_AABTREE, total_size);
        main_header.write(writer)?;

        // Write header sub-chunk
        Self::write_header(tree, writer)?;

        // Write poly indices sub-chunk
        Self::write_poly_indices(tree, writer)?;

        // Write nodes sub-chunk
        Self::write_nodes(tree, writer)?;

        Ok(())
    }

    fn read_header<R: Read>(reader: &mut R) -> io::Result<W3dMeshAABTreeHeader> {
        let mut buf = vec![0u8; std::mem::size_of::<W3dMeshAABTreeHeader>()];
        reader.read_exact(&mut buf)?;

        let node_count = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
        let poly_count = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);

        Ok(W3dMeshAABTreeHeader::new(node_count, poly_count))
    }

    fn read_poly_indices<R: Read>(reader: &mut R, poly_indices: &mut [u32]) -> io::Result<()> {
        for poly_idx in poly_indices.iter_mut() {
            let mut buf = [0u8; 4];
            reader.read_exact(&mut buf)?;
            *poly_idx = u32::from_le_bytes(buf);
        }
        Ok(())
    }

    fn read_nodes<R: Read>(reader: &mut R, nodes: &mut [CullNode]) -> io::Result<()> {
        for node in nodes.iter_mut() {
            let w3d_node = Self::read_node(reader)?;
            *node = w3d_node.to_cull_node();
        }
        Ok(())
    }

    fn read_node<R: Read>(reader: &mut R) -> io::Result<W3dMeshAABTreeNode> {
        let mut buf = vec![0u8; std::mem::size_of::<W3dMeshAABTreeNode>()];
        reader.read_exact(&mut buf)?;

        let min = [
            f32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]),
            f32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]),
            f32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]),
        ];

        let max = [
            f32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]),
            f32::from_le_bytes([buf[16], buf[17], buf[18], buf[19]]),
            f32::from_le_bytes([buf[20], buf[21], buf[22], buf[23]]),
        ];

        let front_or_poly0 = u32::from_le_bytes([buf[24], buf[25], buf[26], buf[27]]);
        let back_or_poly_count = u32::from_le_bytes([buf[28], buf[29], buf[30], buf[31]]);

        Ok(W3dMeshAABTreeNode {
            min,
            max,
            front_or_poly0,
            back_or_poly_count,
        })
    }

    fn write_header<W: Write>(tree: &AABTree, writer: &mut W) -> io::Result<()> {
        let header = W3dMeshAABTreeHeader::new(tree.node_count as u32, tree.poly_count as u32);
        let size = std::mem::size_of::<W3dMeshAABTreeHeader>() as u32;

        let chunk_header = W3dChunkHeader::new(W3D_CHUNK_AABTREE_HEADER, size);
        chunk_header.write(writer)?;

        writer.write_all(&header.node_count.to_le_bytes())?;
        writer.write_all(&header.poly_count.to_le_bytes())?;

        // Write padding
        for _ in 0..6 {
            writer.write_all(&0u32.to_le_bytes())?;
        }

        Ok(())
    }

    fn write_poly_indices<W: Write>(tree: &AABTree, writer: &mut W) -> io::Result<()> {
        let size = (tree.poly_count * std::mem::size_of::<u32>()) as u32;
        let chunk_header = W3dChunkHeader::new(W3D_CHUNK_AABTREE_POLYINDICES, size);
        chunk_header.write(writer)?;

        for &poly_idx in &tree.poly_indices {
            writer.write_all(&poly_idx.to_le_bytes())?;
        }

        Ok(())
    }

    fn write_nodes<W: Write>(tree: &AABTree, writer: &mut W) -> io::Result<()> {
        let size = (tree.node_count * std::mem::size_of::<W3dMeshAABTreeNode>()) as u32;
        let chunk_header = W3dChunkHeader::new(W3D_CHUNK_AABTREE_NODES, size);
        chunk_header.write(writer)?;

        for node in &tree.nodes {
            Self::write_node(node, writer)?;
        }

        Ok(())
    }

    fn write_node<W: Write>(node: &CullNode, writer: &mut W) -> io::Result<()> {
        let w3d_node = W3dMeshAABTreeNode::from_cull_node(node);

        writer.write_all(&w3d_node.min[0].to_le_bytes())?;
        writer.write_all(&w3d_node.min[1].to_le_bytes())?;
        writer.write_all(&w3d_node.min[2].to_le_bytes())?;

        writer.write_all(&w3d_node.max[0].to_le_bytes())?;
        writer.write_all(&w3d_node.max[1].to_le_bytes())?;
        writer.write_all(&w3d_node.max[2].to_le_bytes())?;

        writer.write_all(&w3d_node.front_or_poly0.to_le_bytes())?;
        writer.write_all(&w3d_node.back_or_poly_count.to_le_bytes())?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_header_roundtrip() {
        let header = W3dChunkHeader::new(W3D_CHUNK_AABTREE, 1024);

        let mut buf = Vec::new();
        header.write(&mut buf).unwrap();

        let mut cursor = std::io::Cursor::new(&buf);
        let read_header = W3dChunkHeader::read(&mut cursor).unwrap();

        assert_eq!(header.chunk_type, read_header.chunk_type);
        assert_eq!(header.chunk_size, read_header.chunk_size);
    }

    #[test]
    fn test_aabtree_save_load_roundtrip() {
        // Create a simple tree
        let mut tree = AABTree::new();
        tree.node_count = 3;
        tree.poly_count = 2;
        tree.nodes = vec![
            CullNode {
                min: Vec3::new(-1.0, -1.0, -1.0),
                max: Vec3::new(1.0, 1.0, 1.0),
                front_or_poly0: 0 | 0x8000_0000,
                back_or_poly_count: 2,
            },
            CullNode::new(),
            CullNode::new(),
        ];
        tree.poly_indices = vec![0, 1];

        // Save to buffer
        let mut buf = Vec::new();
        AABTreeIO::save_w3d(&tree, &mut buf).unwrap();

        // Load from buffer
        let mut cursor = std::io::Cursor::new(&buf);
        let loaded_tree = AABTreeIO::load_w3d(&mut cursor).unwrap();

        assert_eq!(tree.node_count, loaded_tree.node_count);
        assert_eq!(tree.poly_count, loaded_tree.poly_count);
        assert_eq!(tree.poly_indices, loaded_tree.poly_indices);

        // Check first node data
        assert_eq!(tree.nodes[0].min, loaded_tree.nodes[0].min);
        assert_eq!(tree.nodes[0].max, loaded_tree.nodes[0].max);
    }
}
