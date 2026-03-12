use ww3d_core::w3d_format::*;
use std::mem::{size_of, offset_of};

fn main() {
    println!("W3dAABTreeHeader: size={}, node_count_offset={}, poly_count_offset={}, padding_offset={}", 
        size_of::<W3dAABTreeHeader>(),
        offset_of!(W3dAABTreeHeader, node_count),
        offset_of!(W3dAABTreeHeader, poly_count),
        offset_of!(W3dAABTreeHeader, padding)
    );
    
    println!("W3dAABTreeNode: size={}, min_offset={}, max_offset={}, front_offset={}, back_offset={}", 
        size_of::<W3dAABTreeNode>(),
        offset_of!(W3dAABTreeNode, min),
        offset_of!(W3dAABTreeNode, max),
        offset_of!(W3dAABTreeNode, front_or_poly0),
        offset_of!(W3dAABTreeNode, back_or_poly_count)
    );

    println!("W3dMeshHeader3Struct: size={}", size_of::<W3dMeshHeader3Struct>());
    println!("W3dVertInfStruct: size={}, bone_idx_offset={}, pad_offset={}", 
        size_of::<W3dVertInfStruct>(),
        offset_of!(W3dVertInfStruct, bone_idx),
        offset_of!(W3dVertInfStruct, pad)
    );
}
