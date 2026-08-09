//! ImagePacker library surface (no gpui) for C++-matching pack/parse tests.

pub mod packer;
pub mod texture_page;

pub use texture_page::{
    add_image_data_tga, extend_image_edges, generate_mapped_image_ini,
    generate_mapped_image_ini_from_pages, pack_named_images_to_pages, ImagePlacement, ImageToPack,
    MappedImageIniEntry, MappedImageIniPage, PackedAtlasPage, PackedAtlasSprite, PageStatus,
    TexturePage, FIT_XBORDER_LEFT, FIT_XBORDER_RIGHT, FIT_XGUTTER, FIT_YBORDER_BOTTOM,
    FIT_YBORDER_TOP, FIT_YGUTTER,
};
