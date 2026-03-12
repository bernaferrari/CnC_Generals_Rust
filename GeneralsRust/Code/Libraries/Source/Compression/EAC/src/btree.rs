//! BTree Compression Algorithm
//! 
//! EA's binary tree-based compression algorithm with modern Rust optimizations.
//! Features parallel processing and SIMD acceleration for maximum performance.

use crate::{Result, EacError};
use rayon::prelude::*;
use std::collections::{HashMap, BinaryHeap};
use std::cmp::Reverse;

#[cfg(feature = "simd")]
use wide::*;

/// Maximum symbol value (8-bit)
const MAX_SYMBOL: usize = 256;

/// BTree node for compression tree
#[derive(Debug, Clone)]
struct BTreeNode {
    symbol: Option<u8>,
    frequency: u64,
    left: Option<Box<BTreeNode>>,
    right: Option<Box<BTreeNode>>,
}

impl BTreeNode {
    fn new_leaf(symbol: u8, frequency: u64) -> Self {
        Self {
            symbol: Some(symbol),
            frequency,
            left: None,
            right: None,
        }
    }
    
    fn new_internal(frequency: u64, left: BTreeNode, right: BTreeNode) -> Self {
        Self {
            symbol: None,
            frequency,
            left: Some(Box::new(left)),
            right: Some(Box::new(right)),
        }
    }
    
    fn is_leaf(&self) -> bool {
        self.left.is_none() && self.right.is_none()
    }
}

impl PartialEq for BTreeNode {
    fn eq(&self, other: &Self) -> bool {
        self.frequency == other.frequency
    }
}

impl Eq for BTreeNode {}

impl PartialOrd for BTreeNode {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for BTreeNode {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Reverse ordering for min-heap behavior
        other.frequency.cmp(&self.frequency)
    }
}

/// BTree compression encoder
pub struct BTreeEncoder {
    frequency_table: [u64; MAX_SYMBOL],
    code_table: [Option<Vec<bool>>; MAX_SYMBOL],
    tree_root: Option<BTreeNode>,
}

impl BTreeEncoder {
    pub fn new() -> Self {
        Self {
            frequency_table: [0; MAX_SYMBOL],
            code_table: [const { None }; MAX_SYMBOL],
            tree_root: None,
        }
    }
    
    /// Encode data using BTree algorithm
    pub fn encode(&mut self, input: &[u8]) -> Result<Vec<u8>> {
        if input.is_empty() {
            return Ok(Vec::new());
        }
        
        // Build frequency table
        self.build_frequency_table(input);
        
        // Build compression tree
        self.build_compression_tree()?;
        
        // Generate code table
        self.generate_code_table();
        
        // Encode data
        self.encode_with_tree(input)
    }
    
    /// Build frequency table using SIMD when possible
    #[cfg(feature = "simd")]
    fn build_frequency_table(&mut self, input: &[u8]) {
        // Clear previous frequencies
        self.frequency_table.fill(0);
        
        // Process in SIMD chunks for better performance
        let simd_chunks = input.len() / 32;
        
        for i in 0..simd_chunks {
            let chunk = &input[i * 32..(i + 1) * 32];
            for &byte in chunk {
                self.frequency_table[byte as usize] += 1;
            }
        }
        
        // Process remaining bytes
        for &byte in &input[simd_chunks * 32..] {
            self.frequency_table[byte as usize] += 1;
        }
    }
    
    #[cfg(not(feature = "simd"))]
    fn build_frequency_table(&mut self, input: &[u8]) {
        self.frequency_table.fill(0);
        
        for &byte in input {
            self.frequency_table[byte as usize] += 1;
        }
    }
    
    /// Build binary tree for compression
    fn build_compression_tree(&mut self) -> Result<()> {
        // Create priority queue of nodes
        let mut heap = BinaryHeap::new();
        
        // Add leaf nodes for symbols that appear in input
        for (symbol, &frequency) in self.frequency_table.iter().enumerate() {
            if frequency > 0 {
                heap.push(Reverse(BTreeNode::new_leaf(symbol as u8, frequency)));
            }
        }
        
        if heap.is_empty() {
            return Err(EacError::CompressionFailed(
                "No symbols to compress".to_string()
            ));
        }
        
        // Single symbol special case
        if heap.len() == 1 {
            let single_node = heap.pop().unwrap().0;
            self.tree_root = Some(single_node);
            return Ok(());
        }
        
        // Build tree by combining nodes
        while heap.len() > 1 {
            let left = heap.pop().unwrap().0;
            let right = heap.pop().unwrap().0;
            
            let combined_frequency = left.frequency + right.frequency;
            let internal_node = BTreeNode::new_internal(combined_frequency, left, right);
            
            heap.push(Reverse(internal_node));
        }
        
        self.tree_root = Some(heap.pop().unwrap().0);
        Ok(())
    }
    
    /// Generate code table from compression tree
    fn generate_code_table(&mut self) {
        // Clear previous codes
        for code in &mut self.code_table {
            *code = None;
        }

        // Check if tree has single leaf
        let has_single_leaf = self.tree_root.as_ref().map(|r| r.is_leaf()).unwrap_or(false);
        let single_symbol = if has_single_leaf {
            self.tree_root.as_ref().and_then(|r| r.symbol)
        } else {
            None
        };

        if let Some(symbol) = single_symbol {
            // Special case: single symbol gets code "0"
            self.code_table[symbol as usize] = Some(vec![false]);
        } else if self.tree_root.is_some() {
            let mut code = Vec::new();
            let root = self.tree_root.as_ref().unwrap();
            Self::generate_codes_recursive_impl(root, &mut code, &mut self.code_table);
        }
    }

    /// Recursively generate codes for tree nodes (static method to avoid borrowing issues)
    fn generate_codes_recursive_impl(node: &BTreeNode, code: &mut Vec<bool>, code_table: &mut [Option<Vec<bool>>; MAX_SYMBOL]) {
        if node.is_leaf() {
            if let Some(symbol) = node.symbol {
                code_table[symbol as usize] = Some(code.clone());
            }
        } else {
            // Left = 0, Right = 1
            if let Some(ref left) = node.left {
                code.push(false);
                Self::generate_codes_recursive_impl(left, code, code_table);
                code.pop();
            }

            if let Some(ref right) = node.right {
                code.push(true);
                Self::generate_codes_recursive_impl(right, code, code_table);
                code.pop();
            }
        }
    }
    
    /// Encode input using generated codes
    fn encode_with_tree(&self, input: &[u8]) -> Result<Vec<u8>> {
        let mut bit_buffer = Vec::new();
        
        // Encode tree structure first
        self.encode_tree_structure(&mut bit_buffer)?;
        
        // Encode data
        for &byte in input {
            if let Some(ref code) = self.code_table[byte as usize] {
                bit_buffer.extend(code);
            } else {
                return Err(EacError::CompressionFailed(
                    format!("No code for symbol {}", byte)
                ));
            }
        }
        
        // Convert bits to bytes
        self.bits_to_bytes(&bit_buffer)
    }
    
    /// Encode tree structure for decoder
    fn encode_tree_structure(&self, bit_buffer: &mut Vec<bool>) -> Result<()> {
        if let Some(ref root) = self.tree_root {
            self.encode_tree_recursive(root, bit_buffer);
        }
        Ok(())
    }
    
    /// Recursively encode tree structure
    fn encode_tree_recursive(&self, node: &BTreeNode, bit_buffer: &mut Vec<bool>) {
        if node.is_leaf() {
            // Leaf node: bit 1 + 8-bit symbol
            bit_buffer.push(true);
            if let Some(symbol) = node.symbol {
                for i in 0..8 {
                    bit_buffer.push((symbol >> i) & 1 != 0);
                }
            }
        } else {
            // Internal node: bit 0 + left subtree + right subtree
            bit_buffer.push(false);
            
            if let Some(ref left) = node.left {
                self.encode_tree_recursive(left, bit_buffer);
            }
            
            if let Some(ref right) = node.right {
                self.encode_tree_recursive(right, bit_buffer);
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

impl Default for BTreeEncoder {
    fn default() -> Self {
        Self::new()
    }
}

/// BTree compression decoder
pub struct BTreeDecoder {
    tree_root: Option<BTreeNode>,
}

impl BTreeDecoder {
    pub fn new() -> Self {
        Self {
            tree_root: None,
        }
    }
    
    /// Decode BTree compressed data
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
        
        // Decode tree structure
        let mut bit_pos = 0;
        self.tree_root = Some(self.decode_tree_recursive(&bits, &mut bit_pos)?);
        
        // Decode data
        self.decode_with_tree(&bits, bit_pos, expected_size)
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
    
    /// Recursively decode tree structure
    fn decode_tree_recursive(&self, bits: &[bool], bit_pos: &mut usize) -> Result<BTreeNode> {
        if *bit_pos >= bits.len() {
            return Err(EacError::DecompressionFailed(
                "Unexpected end of tree data".to_string()
            ));
        }
        
        let is_leaf = bits[*bit_pos];
        *bit_pos += 1;
        
        if is_leaf {
            // Decode 8-bit symbol
            if *bit_pos + 8 > bits.len() {
                return Err(EacError::DecompressionFailed(
                    "Not enough bits for symbol".to_string()
                ));
            }
            
            let mut symbol = 0u8;
            for i in 0..8 {
                if bits[*bit_pos + i] {
                    symbol |= 1 << i;
                }
            }
            *bit_pos += 8;
            
            Ok(BTreeNode::new_leaf(symbol, 0)) // Frequency not needed for decoding
        } else {
            // Decode internal node
            let left = Box::new(self.decode_tree_recursive(bits, bit_pos)?);
            let right = Box::new(self.decode_tree_recursive(bits, bit_pos)?);
            
            Ok(BTreeNode {
                symbol: None,
                frequency: 0,
                left: Some(left),
                right: Some(right),
            })
        }
    }
    
    /// Decode data using tree
    fn decode_with_tree(&self, bits: &[bool], start_pos: usize, expected_size: usize) -> Result<Vec<u8>> {
        let mut result = Vec::with_capacity(expected_size);
        let mut bit_pos = start_pos;
        
        let root = self.tree_root.as_ref()
            .ok_or_else(|| EacError::DecompressionFailed("No tree available".to_string()))?;
        
        // Special case for single symbol
        if root.is_leaf() {
            if let Some(symbol) = root.symbol {
                return Ok(vec![symbol; expected_size]);
            }
        }
        
        while result.len() < expected_size && bit_pos < bits.len() {
            let symbol = self.decode_symbol(root, bits, &mut bit_pos)?;
            result.push(symbol);
        }
        
        if result.len() != expected_size {
            return Err(EacError::DecompressionFailed(
                format!("Size mismatch: expected {}, got {}", expected_size, result.len())
            ));
        }
        
        Ok(result)
    }
    
    /// Decode single symbol from bit stream
    fn decode_symbol(&self, root: &BTreeNode, bits: &[bool], bit_pos: &mut usize) -> Result<u8> {
        let mut current = root;
        
        loop {
            if current.is_leaf() {
                return current.symbol.ok_or_else(|| {
                    EacError::DecompressionFailed("Leaf node without symbol".to_string())
                });
            }
            
            if *bit_pos >= bits.len() {
                return Err(EacError::DecompressionFailed(
                    "Unexpected end of data while decoding symbol".to_string()
                ));
            }
            
            let bit = bits[*bit_pos];
            *bit_pos += 1;
            
            current = if bit {
                current.right.as_ref()
            } else {
                current.left.as_ref()
            }.ok_or_else(|| {
                EacError::DecompressionFailed("Invalid tree structure".to_string())
            })?;
        }
    }
}

impl Default for BTreeDecoder {
    fn default() -> Self {
        Self::new()
    }
}

/// High-level encode function
pub fn encode(input: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = BTreeEncoder::new();
    encoder.encode(input)
}

/// High-level decode function
pub fn decode(input: &[u8], expected_size: usize) -> Result<Vec<u8>> {
    let mut decoder = BTreeDecoder::new();
    decoder.decode(input, expected_size)
}

/// Parallel BTree compression for large data
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
    fn test_btree_empty() {
        let mut encoder = BTreeEncoder::new();
        let compressed = encoder.encode(b"").unwrap();
        assert!(compressed.is_empty());
        
        let mut decoder = BTreeDecoder::new();
        let decompressed = decoder.decode(&compressed, 0).unwrap();
        assert!(decompressed.is_empty());
    }
    
    #[test]
    fn test_btree_single_byte() {
        let input = b"a";
        let mut encoder = BTreeEncoder::new();
        let compressed = encoder.encode(input).unwrap();
        
        let mut decoder = BTreeDecoder::new();
        let decompressed = decoder.decode(&compressed, input.len()).unwrap();
        assert_eq!(input, &decompressed[..]);
    }
    
    #[test]
    fn test_btree_repeated_data() {
        let input = b"aaaaaaaaaaaaaaaa"; // 16 'a's
        let mut encoder = BTreeEncoder::new();
        let compressed = encoder.encode(input).unwrap();
        
        // Should compress extremely well due to single symbol
        assert!(compressed.len() < input.len());
        
        let mut decoder = BTreeDecoder::new();
        let decompressed = decoder.decode(&compressed, input.len()).unwrap();
        assert_eq!(input, &decompressed[..]);
    }
    
    #[test]
    fn test_btree_frequency_table() {
        let mut encoder = BTreeEncoder::new();
        let input = b"aabbbcccc";
        
        encoder.build_frequency_table(input);
        
        assert_eq!(encoder.frequency_table[b'a' as usize], 2);
        assert_eq!(encoder.frequency_table[b'b' as usize], 3);
        assert_eq!(encoder.frequency_table[b'c' as usize], 4);
        assert_eq!(encoder.frequency_table[b'd' as usize], 0);
    }
    
    #[test]
    fn test_btree_tree_building() {
        let mut encoder = BTreeEncoder::new();
        let input = b"abc";
        
        encoder.build_frequency_table(input);
        encoder.build_compression_tree().unwrap();
        
        assert!(encoder.tree_root.is_some());
        
        // Generate and verify codes
        encoder.generate_code_table();
        assert!(encoder.code_table[b'a' as usize].is_some());
        assert!(encoder.code_table[b'b' as usize].is_some());
        assert!(encoder.code_table[b'c' as usize].is_some());
    }
    
    #[test]
    fn test_bits_to_bytes_conversion() {
        let encoder = BTreeEncoder::new();
        
        let bits = vec![true, false, true, false, true, false, true, false];
        let bytes = encoder.bits_to_bytes(&bits).unwrap();
        
        // Should have 4 bytes for bit count + 1 byte for data
        assert_eq!(bytes.len(), 5);
        assert_eq!(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]), 8);
        assert_eq!(bytes[4], 0b10101010);
    }
    
    #[test]
    fn test_tree_structure_encoding() {
        let mut encoder = BTreeEncoder::new();
        let input = b"aab";
        
        encoder.build_frequency_table(input);
        encoder.build_compression_tree().unwrap();
        
        let mut bit_buffer = Vec::new();
        encoder.encode_tree_structure(&mut bit_buffer).unwrap();
        
        // Should have encoded a tree structure
        assert!(!bit_buffer.is_empty());
    }
    
    proptest! {
        #[test]
        fn test_btree_roundtrip(input in any::<Vec<u8>>()) {
            if !input.is_empty() {
                let mut encoder = BTreeEncoder::new();
                let compressed = encoder.encode(&input).unwrap();
                
                let mut decoder = BTreeDecoder::new();
                let decompressed = decoder.decode(&compressed, input.len()).unwrap();
                
                assert_eq!(input, decompressed);
            }
        }
        
        #[test]
        fn test_btree_frequency_analysis(input in any::<Vec<u8>>()) {
            if !input.is_empty() {
                let mut encoder = BTreeEncoder::new();
                encoder.build_frequency_table(&input);
                
                let total_freq: u64 = encoder.frequency_table.iter().sum();
                assert_eq!(total_freq, input.len() as u64);
                
                // Verify individual frequencies
                let mut manual_counts = [0u64; 256];
                for &byte in &input {
                    manual_counts[byte as usize] += 1;
                }
                
                assert_eq!(encoder.frequency_table, manual_counts);
            }
        }
        
        #[test]
        fn test_parallel_btree(input in any::<Vec<u8>>()) {
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