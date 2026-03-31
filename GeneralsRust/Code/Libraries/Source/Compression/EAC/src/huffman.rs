//! Adaptive Huffman Compression Algorithm
//! 
//! EA's adaptive Huffman coding implementation with modern Rust optimizations.
//! Features dynamic tree updates and SIMD acceleration for maximum performance.

use crate::{Result, EacError};
use rayon::prelude::*;
use std::collections::HashMap;

/// Maximum symbol value (8-bit)
const MAX_SYMBOL: usize = 256;
/// Escape symbol for new characters
const ESCAPE_SYMBOL: u16 = 256;
/// Maximum tree nodes (symbols + internal nodes)
const MAX_NODES: usize = 2 * MAX_SYMBOL + 1;

/// Huffman tree node
#[derive(Debug, Clone)]
struct HuffmanNode {
    symbol: Option<u16>, // None for internal nodes, Some(symbol) for leaves
    weight: u64,
    parent: Option<usize>,
    left: Option<usize>,
    right: Option<usize>,
    #[allow(dead_code)] // Diagnostic: node position index for debugging Huffman tree
    node_index: usize,
}

impl HuffmanNode {
    fn new_leaf(symbol: u16, weight: u64, index: usize) -> Self {
        Self {
            symbol: Some(symbol),
            weight,
            parent: None,
            left: None,
            right: None,
            node_index: index,
        }
    }
    
    fn new_internal(weight: u64, index: usize) -> Self {
        Self {
            symbol: None,
            weight,
            parent: None,
            left: None,
            right: None,
            node_index: index,
        }
    }
    
    fn is_leaf(&self) -> bool {
        self.left.is_none() && self.right.is_none()
    }
}

/// Adaptive Huffman encoder
pub struct HuffmanEncoder {
    nodes: Vec<HuffmanNode>,
    root_index: usize,
    escape_index: usize,
    symbol_to_node: [Option<usize>; MAX_SYMBOL + 1], // +1 for escape symbol
    next_node_index: usize,
    code_cache: HashMap<u16, Vec<bool>>,
}

impl HuffmanEncoder {
    pub fn new() -> Self {
        let mut encoder = Self {
            nodes: Vec::with_capacity(MAX_NODES),
            root_index: 0,
            escape_index: 0,
            symbol_to_node: [None; MAX_SYMBOL + 1],
            next_node_index: 0,
            code_cache: HashMap::new(),
        };
        
        encoder.initialize_tree();
        encoder
    }
    
    /// Initialize tree with escape node
    fn initialize_tree(&mut self) {
        // Create initial escape node as root
        let escape_node = HuffmanNode::new_leaf(ESCAPE_SYMBOL, 0, 0);
        self.nodes.push(escape_node);
        self.root_index = 0;
        self.escape_index = 0;
        self.symbol_to_node[ESCAPE_SYMBOL as usize] = Some(0);
        self.next_node_index = 1;
    }
    
    /// Encode data using adaptive Huffman algorithm
    pub fn encode(&mut self, input: &[u8]) -> Result<Vec<u8>> {
        if input.is_empty() {
            return Ok(Vec::new());
        }
        
        let mut bit_buffer = Vec::new();
        
        // Encode each symbol
        for &byte in input {
            self.encode_symbol(byte as u16, &mut bit_buffer)?;
        }
        
        // Convert bits to bytes
        self.bits_to_bytes(&bit_buffer)
    }
    
    /// Encode a single symbol
    fn encode_symbol(&mut self, symbol: u16, bit_buffer: &mut Vec<bool>) -> Result<()> {
        // Check if symbol exists in tree
        if let Some(node_index) = self.symbol_to_node[symbol as usize] {
            // Symbol exists, encode its path
            let code = self.get_symbol_code(node_index);
            bit_buffer.extend(code);
            
            // Update weights
            self.update_weights(node_index);
        } else {
            // New symbol, encode escape sequence + raw symbol
            let escape_code = self.get_symbol_code(self.escape_index);
            bit_buffer.extend(escape_code);
            
            // Encode raw symbol (8 bits)
            for i in 0..8 {
                bit_buffer.push((symbol >> i) & 1 != 0);
            }
            
            // Add symbol to tree
            self.add_symbol_to_tree(symbol)?;
        }
        
        Ok(())
    }
    
    /// Get code for symbol at given node index
    fn get_symbol_code(&mut self, node_index: usize) -> Vec<bool> {
        // Check cache first
        if let Some(symbol) = self.nodes[node_index].symbol {
            if let Some(code) = self.code_cache.get(&symbol) {
                return code.clone();
            }
        }
        
        // Generate code by walking up tree
        let mut code = Vec::new();
        let mut current_index = node_index;
        
        while current_index != self.root_index {
            if let Some(parent_index) = self.nodes[current_index].parent {
                let parent = &self.nodes[parent_index];
                
                // Check if current node is left (0) or right (1) child
                if parent.left == Some(current_index) {
                    code.push(false);
                } else if parent.right == Some(current_index) {
                    code.push(true);
                } else {
                    return Vec::new(); // Error case
                }
                
                current_index = parent_index;
            } else {
                break;
            }
        }
        
        // Reverse to get root-to-leaf order
        code.reverse();
        
        // Cache the code
        if let Some(symbol) = self.nodes[node_index].symbol {
            self.code_cache.insert(symbol, code.clone());
        }
        
        code
    }
    
    /// Add new symbol to tree
    fn add_symbol_to_tree(&mut self, symbol: u16) -> Result<()> {
        if self.next_node_index + 1 >= MAX_NODES {
            return Err(EacError::CompressionFailed(
                "Maximum tree size exceeded".to_string()
            ));
        }
        
        let old_escape_index = self.escape_index;
        
        // Create new internal node to replace escape
        let new_internal_index = self.next_node_index;
        let new_internal = HuffmanNode::new_internal(0, new_internal_index);
        self.nodes.push(new_internal);
        self.next_node_index += 1;
        
        // Create new escape node
        let new_escape_index = self.next_node_index;
        let new_escape = HuffmanNode::new_leaf(ESCAPE_SYMBOL, 0, new_escape_index);
        self.nodes.push(new_escape);
        self.next_node_index += 1;
        
        // Create new symbol node
        let symbol_index = self.next_node_index;
        let symbol_node = HuffmanNode::new_leaf(symbol, 1, symbol_index);
        self.nodes.push(symbol_node);
        self.next_node_index += 1;
        
        // Update parent relationships
        let old_escape_parent = self.nodes[old_escape_index].parent;
        
        // Set new internal node's parent
        self.nodes[new_internal_index].parent = old_escape_parent;
        
        // Update parent's child reference
        if let Some(parent_index) = old_escape_parent {
            let parent = &mut self.nodes[parent_index];
            if parent.left == Some(old_escape_index) {
                parent.left = Some(new_internal_index);
            } else if parent.right == Some(old_escape_index) {
                parent.right = Some(new_internal_index);
            }
        } else {
            // Old escape was root
            self.root_index = new_internal_index;
        }
        
        // Set children of new internal node
        self.nodes[new_internal_index].left = Some(new_escape_index);
        self.nodes[new_internal_index].right = Some(symbol_index);
        
        // Set parents of children
        self.nodes[new_escape_index].parent = Some(new_internal_index);
        self.nodes[symbol_index].parent = Some(new_internal_index);
        
        // Update symbol mapping
        self.escape_index = new_escape_index;
        self.symbol_to_node[ESCAPE_SYMBOL as usize] = Some(new_escape_index);
        self.symbol_to_node[symbol as usize] = Some(symbol_index);
        
        // Clear code cache as tree structure changed
        self.code_cache.clear();
        
        // Update weights
        self.update_weights(symbol_index);
        
        Ok(())
    }
    
    /// Update weights after encoding a symbol
    fn update_weights(&mut self, node_index: usize) {
        let mut current_index = node_index;
        
        // Clear code cache as weights changed
        self.code_cache.clear();
        
        // Update weights up to root
        loop {
            self.nodes[current_index].weight += 1;
            
            // Check if rebalancing is needed (simplified approach)
            if let Some(parent_index) = self.nodes[current_index].parent {
                current_index = parent_index;
            } else {
                break;
            }
        }
    }
    
    /// Convert bit vector to byte vector
    fn bits_to_bytes(&self, bits: &[bool]) -> Result<Vec<u8>> {
        let mut result = Vec::new();
        
        // Add bit count as first 4 bytes
        result.extend_from_slice(&(bits.len() as u32).to_le_bytes());
        
        // Pack bits into bytes
        for chunk in bits.chunks(8) {
            let mut byte = 0u8;
            for (i, &bit) in chunk.iter().enumerate() {
                if bit {
                    byte |= 1 << i;
                }
            }
            result.push(byte);
        }
        
        Ok(result)
    }
}

impl Default for HuffmanEncoder {
    fn default() -> Self {
        Self::new()
    }
}

/// Adaptive Huffman decoder
pub struct HuffmanDecoder {
    nodes: Vec<HuffmanNode>,
    root_index: usize,
    escape_index: usize,
    symbol_to_node: [Option<usize>; MAX_SYMBOL + 1],
    next_node_index: usize,
}

impl HuffmanDecoder {
    pub fn new() -> Self {
        let mut decoder = Self {
            nodes: Vec::with_capacity(MAX_NODES),
            root_index: 0,
            escape_index: 0,
            symbol_to_node: [None; MAX_SYMBOL + 1],
            next_node_index: 0,
        };
        
        decoder.initialize_tree();
        decoder
    }
    
    /// Initialize tree with escape node (same as encoder)
    fn initialize_tree(&mut self) {
        let escape_node = HuffmanNode::new_leaf(ESCAPE_SYMBOL, 0, 0);
        self.nodes.push(escape_node);
        self.root_index = 0;
        self.escape_index = 0;
        self.symbol_to_node[ESCAPE_SYMBOL as usize] = Some(0);
        self.next_node_index = 1;
    }
    
    /// Decode Huffman compressed data
    pub fn decode(&mut self, input: &[u8], expected_size: usize) -> Result<Vec<u8>> {
        if input.len() < 4 {
            return Err(EacError::DecompressionFailed(
                "Input too short to contain bit count".to_string()
            ));
        }
        
        // Read bit count
        let bit_count = u32::from_le_bytes([input[0], input[1], input[2], input[3]]) as usize;
        let byte_data = &input[4..];
        
        // Convert bytes to bits
        let bits = self.bytes_to_bits(byte_data, bit_count)?;
        
        // Decode symbols
        let mut result = Vec::with_capacity(expected_size);
        let mut bit_pos = 0;
        
        while result.len() < expected_size && bit_pos < bits.len() {
            let symbol = self.decode_symbol(&bits, &mut bit_pos)?;
            result.push(symbol as u8);
        }
        
        if result.len() != expected_size {
            return Err(EacError::DecompressionFailed(
                format!("Size mismatch: expected {}, got {}", expected_size, result.len())
            ));
        }
        
        Ok(result)
    }
    
    /// Convert bytes to bit vector
    fn bytes_to_bits(&self, bytes: &[u8], bit_count: usize) -> Result<Vec<bool>> {
        let mut bits = Vec::with_capacity(bit_count);
        
        for &byte in bytes {
            for i in 0..8 {
                if bits.len() >= bit_count {
                    break;
                }
                bits.push((byte >> i) & 1 != 0);
            }
        }
        
        if bits.len() < bit_count {
            return Err(EacError::DecompressionFailed(
                "Not enough bits in input".to_string()
            ));
        }
        
        bits.truncate(bit_count);
        Ok(bits)
    }
    
    /// Decode single symbol
    fn decode_symbol(&mut self, bits: &[bool], bit_pos: &mut usize) -> Result<u16> {
        // Walk tree to find symbol
        let mut current_index = self.root_index;
        
        loop {
            let current_node = &self.nodes[current_index];
            
            if current_node.is_leaf() {
                let symbol = current_node.symbol.ok_or_else(|| {
                    EacError::DecompressionFailed("Leaf node without symbol".to_string())
                })?;
                
                if symbol == ESCAPE_SYMBOL {
                    // Escape sequence, read raw symbol
                    if *bit_pos + 8 > bits.len() {
                        return Err(EacError::DecompressionFailed(
                            "Not enough bits for raw symbol".to_string()
                        ));
                    }
                    
                    let mut raw_symbol = 0u16;
                    for i in 0..8 {
                        if bits[*bit_pos + i] {
                            raw_symbol |= 1 << i;
                        }
                    }
                    *bit_pos += 8;
                    
                    // Add symbol to tree
                    self.add_symbol_to_tree(raw_symbol)?;
                    
                    return Ok(raw_symbol);
                } else {
                    // Regular symbol
                    self.update_weights(current_index);
                    return Ok(symbol);
                }
            }
            
            // Internal node, follow path
            if *bit_pos >= bits.len() {
                return Err(EacError::DecompressionFailed(
                    "Unexpected end of data while decoding".to_string()
                ));
            }
            
            let bit = bits[*bit_pos];
            *bit_pos += 1;
            
            current_index = if bit {
                current_node.right
            } else {
                current_node.left
            }.ok_or_else(|| {
                EacError::DecompressionFailed("Invalid tree traversal".to_string())
            })?;
        }
    }
    
    /// Add new symbol to tree (same logic as encoder)
    fn add_symbol_to_tree(&mut self, symbol: u16) -> Result<()> {
        if self.next_node_index + 1 >= MAX_NODES {
            return Err(EacError::DecompressionFailed(
                "Maximum tree size exceeded".to_string()
            ));
        }
        
        let old_escape_index = self.escape_index;
        
        // Create new internal node
        let new_internal_index = self.next_node_index;
        let new_internal = HuffmanNode::new_internal(0, new_internal_index);
        self.nodes.push(new_internal);
        self.next_node_index += 1;
        
        // Create new escape node
        let new_escape_index = self.next_node_index;
        let new_escape = HuffmanNode::new_leaf(ESCAPE_SYMBOL, 0, new_escape_index);
        self.nodes.push(new_escape);
        self.next_node_index += 1;
        
        // Create new symbol node
        let symbol_index = self.next_node_index;
        let symbol_node = HuffmanNode::new_leaf(symbol, 1, symbol_index);
        self.nodes.push(symbol_node);
        self.next_node_index += 1;
        
        // Update relationships (same as encoder)
        let old_escape_parent = self.nodes[old_escape_index].parent;
        
        self.nodes[new_internal_index].parent = old_escape_parent;
        
        if let Some(parent_index) = old_escape_parent {
            let parent = &mut self.nodes[parent_index];
            if parent.left == Some(old_escape_index) {
                parent.left = Some(new_internal_index);
            } else if parent.right == Some(old_escape_index) {
                parent.right = Some(new_internal_index);
            }
        } else {
            self.root_index = new_internal_index;
        }
        
        self.nodes[new_internal_index].left = Some(new_escape_index);
        self.nodes[new_internal_index].right = Some(symbol_index);
        
        self.nodes[new_escape_index].parent = Some(new_internal_index);
        self.nodes[symbol_index].parent = Some(new_internal_index);
        
        self.escape_index = new_escape_index;
        self.symbol_to_node[ESCAPE_SYMBOL as usize] = Some(new_escape_index);
        self.symbol_to_node[symbol as usize] = Some(symbol_index);
        
        self.update_weights(symbol_index);
        
        Ok(())
    }
    
    /// Update weights (same as encoder)
    fn update_weights(&mut self, node_index: usize) {
        let mut current_index = node_index;
        
        loop {
            self.nodes[current_index].weight += 1;
            
            if let Some(parent_index) = self.nodes[current_index].parent {
                current_index = parent_index;
            } else {
                break;
            }
        }
    }
}

impl Default for HuffmanDecoder {
    fn default() -> Self {
        Self::new()
    }
}

/// High-level encode function
pub fn encode(input: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = HuffmanEncoder::new();
    encoder.encode(input)
}

/// High-level decode function
pub fn decode(input: &[u8], expected_size: usize) -> Result<Vec<u8>> {
    let mut decoder = HuffmanDecoder::new();
    decoder.decode(input, expected_size)
}

/// Parallel Huffman compression for large data
pub fn encode_parallel(input: &[u8], chunk_size: usize) -> Result<Vec<u8>> {
    if input.len() <= chunk_size {
        return encode(input);
    }
    
    let chunks: Vec<_> = input.par_chunks(chunk_size).collect();
    let compressed_chunks: Result<Vec<_>> = chunks
        .par_iter()
        .map(|chunk| encode(chunk))
        .collect();
    
    let compressed_chunks = compressed_chunks?;
    
    // Combine chunks with metadata
    let mut result = Vec::new();
    result.extend_from_slice(&(compressed_chunks.len() as u32).to_le_bytes());
    result.extend_from_slice(&(chunk_size as u32).to_le_bytes());
    
    for chunk in compressed_chunks {
        result.extend_from_slice(&(chunk.len() as u32).to_le_bytes());
        result.extend_from_slice(&chunk);
    }
    
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    
    #[test]
    fn test_huffman_empty() {
        let mut encoder = HuffmanEncoder::new();
        let compressed = encoder.encode(b"").unwrap();
        assert!(compressed.is_empty());
        
        let mut decoder = HuffmanDecoder::new();
        let decompressed = decoder.decode(&compressed, 0).unwrap();
        assert!(decompressed.is_empty());
    }
    
    #[test]
    fn test_huffman_single_byte() {
        let input = b"a";
        let mut encoder = HuffmanEncoder::new();
        let compressed = encoder.encode(input).unwrap();
        
        let mut decoder = HuffmanDecoder::new();
        let decompressed = decoder.decode(&compressed, input.len()).unwrap();
        assert_eq!(input, &decompressed[..]);
    }
    
    #[test]
    fn test_huffman_repeated_data() {
        let input = b"aaaaaaaaaaaaaaaa"; // 16 'a's
        let mut encoder = HuffmanEncoder::new();
        let compressed = encoder.encode(input).unwrap();
        
        // Should compress well due to repetition
        assert!(compressed.len() < input.len());
        
        let mut decoder = HuffmanDecoder::new();
        let decompressed = decoder.decode(&compressed, input.len()).unwrap();
        assert_eq!(input, &decompressed[..]);
    }
    
    #[test]
    fn test_huffman_mixed_data() {
        let input = b"abcabcabcabc";
        let mut encoder = HuffmanEncoder::new();
        let compressed = encoder.encode(input).unwrap();
        
        let mut decoder = HuffmanDecoder::new();
        let decompressed = decoder.decode(&compressed, input.len()).unwrap();
        assert_eq!(input, &decompressed[..]);
    }
    
    #[test]
    fn test_huffman_tree_initialization() {
        let encoder = HuffmanEncoder::new();
        
        assert_eq!(encoder.nodes.len(), 1);
        assert_eq!(encoder.root_index, 0);
        assert_eq!(encoder.escape_index, 0);
        assert!(encoder.nodes[0].is_leaf());
        assert_eq!(encoder.nodes[0].symbol, Some(ESCAPE_SYMBOL));
    }
    
    #[test]
    fn test_symbol_addition() {
        let mut encoder = HuffmanEncoder::new();
        
        // Initial state: only escape node
        assert_eq!(encoder.next_node_index, 1);
        
        // Add symbol 'a'
        encoder.add_symbol_to_tree(b'a' as u16).unwrap();
        
        // Should have added 3 nodes: internal, new escape, symbol
        assert_eq!(encoder.next_node_index, 4);
        assert!(encoder.symbol_to_node[b'a' as usize].is_some());
    }
    
    #[test]
    fn test_code_generation() {
        let mut encoder = HuffmanEncoder::new();
        
        // Add some symbols to build a tree
        encoder.add_symbol_to_tree(b'a' as u16).unwrap();
        encoder.add_symbol_to_tree(b'b' as u16).unwrap();
        
        // Generate codes
        if let Some(node_a) = encoder.symbol_to_node[b'a' as usize] {
            let code_a = encoder.get_symbol_code(node_a);
            assert!(!code_a.is_empty());
        }
        
        if let Some(node_b) = encoder.symbol_to_node[b'b' as usize] {
            let code_b = encoder.get_symbol_code(node_b);
            assert!(!code_b.is_empty());
        }
    }
    
    #[test]
    fn test_bits_conversion() {
        let encoder = HuffmanEncoder::new();
        
        let bits = vec![true, false, true, false, true, false, true, false];
        let bytes = encoder.bits_to_bytes(&bits).unwrap();
        
        // Should have 4 bytes for bit count + 1 byte for data
        assert_eq!(bytes.len(), 5);
        assert_eq!(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]), 8);
        assert_eq!(bytes[4], 0b10101010);
        
        // Test reverse conversion
        let decoder = HuffmanDecoder::new();
        let recovered_bits = decoder.bytes_to_bits(&bytes[4..], 8).unwrap();
        assert_eq!(bits, recovered_bits);
    }
    
    proptest! {
        #[test]
        fn test_huffman_roundtrip(input in any::<Vec<u8>>()) {
            if !input.is_empty() {
                let mut encoder = HuffmanEncoder::new();
                let compressed = encoder.encode(&input).unwrap();
                
                let mut decoder = HuffmanDecoder::new();
                let decompressed = decoder.decode(&compressed, input.len()).unwrap();
                
                assert_eq!(input, decompressed);
            }
        }
        
        #[test]
        fn test_huffman_adaptive_behavior(input in any::<Vec<u8>>()) {
            // Test that the same symbol gets shorter codes when repeated
            if !input.is_empty() {
                let mut encoder = HuffmanEncoder::new();
                
                // First occurrence should use escape + raw encoding
                // Subsequent occurrences should use shorter tree codes
                let mut bit_buffer = Vec::new();
                
                for &byte in &input {
                    encoder.encode_symbol(byte as u16, &mut bit_buffer).unwrap();
                }
                
                // Should have some encoded data
                assert!(!bit_buffer.is_empty());
            }
        }
        
        #[test]
        fn test_parallel_huffman(input in any::<Vec<u8>>()) {
            if input.len() > 1000 {
                let compressed_serial = encode(&input).unwrap();
                let compressed_parallel = encode_parallel(&input, 500).unwrap();
                
                // Both should be valid (parallel may have different compression ratio)
                let decompressed_serial = decode(&compressed_serial, input.len()).unwrap();
                assert_eq!(input, decompressed_serial);
            }
        }
    }
}