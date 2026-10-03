use super::*;
use bytemuck::Zeroable;

fn dds_bytes(width: u32, height: u32, mips: u32, fourcc: [u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut header = dds::DdsHeader::zeroed();
    header.size = 124;
    header.flags = 0x1007 | 0x80000 | 0x20000;
    header.width = width;
    header.height = height;
    header.mip_map_count = mips;
    header.pixel_format.size = 32;
    header.pixel_format.flags = 4;
    header.pixel_format.four_cc = u32::from_le_bytes(fourcc);
    header.caps.caps1 = 0x1000;
    let mut bytes = b"DDS ".to_vec();
    bytes.extend_from_slice(bytemuck::bytes_of(&header));
    bytes.extend_from_slice(payload);
    bytes
}

#[test]
fn decoded_dds_keeps_cpp_mip_reduction_and_block_spans() {
    // ddsfile.cpp:81-92 drops the two smallest mips; 127-139 records
    // level offsets/sizes. Moving the codec must not discard retained levels.
    let mut payload = vec![1; 128]; // 16x16 BC1
    payload.extend_from_slice(&[2; 32]); // 8x8 BC1
    payload.extend_from_slice(&[3; 8]); // 4x4 BC1
    payload.extend_from_slice(&[4; 8]); // 2x2, dropped
    payload.extend_from_slice(&[5; 8]); // 1x1, dropped
    let image = load_dds_from_memory(&dds_bytes(16, 16, 5, *b"DXT1", &payload)).unwrap();
    assert_eq!(image.format, ImageFormat::Bc1RgbaUnormSrgb);
    assert_eq!(image.compression, Some(DdsCompression::Dxt1));
    assert_eq!(image.texture_type, DdsTextureType::Texture2D);
    assert_eq!(image.mip_levels, 3);
    assert_eq!(image.level_offsets, [0, 128, 160]);
    assert_eq!(image.level_sizes, [128, 32, 8]);
    assert_eq!(image.get_level_data(0), Some(&payload[..128]));
    assert_eq!(image.get_level_data(1), Some(&payload[128..160]));
    assert_eq!(image.get_level_data(2), Some(&payload[160..168]));
    assert_eq!(image.get_level_data(3), None);
    assert_eq!(image.data, payload[..168]);
}

#[test]
fn decoded_dx10_distinguishes_linear_encoding_before_renderer_choice() {
    // Preserve the existing Rust DX10 extension's color interpretation.
    // Original retail C++ does not support this extension.
    let extended = dds::DdsHeaderDxt10 {
        dxgi_format: 70, // BC1_UNORM, not BC1_UNORM_SRGB
        resource_dimension: 3,
        misc_flag: 0,
        array_size: 1,
        misc_flags2: 0,
    };
    let mut payload = bytemuck::bytes_of(&extended).to_vec();
    payload.extend_from_slice(&[0; 8]);
    let image = load_dds_from_memory(&dds_bytes(4, 4, 1, *b"DX10", &payload)).unwrap();
    assert_eq!(image.format, ImageFormat::Bc1RgbaUnorm);
    assert_eq!(image.compression, Some(DdsCompression::Dxt1));
    assert_eq!(image.data, [0; 8]);
}

#[test]
fn software_dxt_blocks_keep_row_order_and_alpha() {
    // DDSFileClass::Get_4x4_Block decodes blocks into their image position.
    let red = [0x00, 0xf8, 0, 0, 0, 0, 0, 0];
    let blue = [0x1f, 0, 0, 0, 0, 0, 0, 0];
    let blocks = [red, blue].concat();
    let decoded = decode_dxt1(&blocks, 8, 4).unwrap();
    for row in decoded.chunks_exact(8 * 4) {
        assert_eq!(&row[..4], &[255, 0, 0, 255]);
        assert_eq!(&row[16..20], &[0, 0, 255, 255]);
    }
    // DXT1 color code 3 when c0 <= c1 is transparent.
    let transparent = [0, 0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff];
    assert_eq!(decode_dxt1(&transparent, 4, 4).unwrap(), vec![0; 64]);
    // Explicit DXT3 alpha 0xa expands to 170, independently of RGB.
    let mut dxt3 = [0xaa; 16];
    dxt3[8..].copy_from_slice(&red);
    assert_eq!(&decode_dxt3(&dxt3, 4, 4).unwrap()[..4], &[255, 0, 0, 170]);
    // DXT5 alpha table selects alpha0 (200) for all zero indices.
    let mut dxt5 = [0; 16];
    dxt5[0] = 200;
    dxt5[1] = 100;
    dxt5[8..].copy_from_slice(&red);
    assert_eq!(&decode_dxt5(&dxt5, 4, 4).unwrap()[..4], &[255, 0, 0, 200]);
}

fn tga_bytes(descriptor: u8, image_type: u8, bits: u8, pixels: &[u8]) -> Vec<u8> {
    let mut header = tga::TgaHeader::zeroed();
    header.width = 2;
    header.height = 2;
    header.image_type = image_type;
    header.bits_per_pixel = bits;
    header.image_descriptor = descriptor;
    let mut bytes = bytemuck::bytes_of(&header).to_vec();
    bytes.extend_from_slice(pixels);
    bytes
}

#[test]
fn tga_origin_and_bgr_channels_are_decoded_before_upload() {
    // textureloader.cpp:507-514 separates Targa image origin from GPU format.
    // Top-origin and bottom-origin spellings decode to identical CPU rows.
    let top = [0, 0, 255, 0, 255, 0, 255, 0, 0, 0, 255, 255];
    let bottom = [255, 0, 0, 0, 255, 255, 0, 0, 255, 0, 255, 0];
    let a = load_tga_from_memory(&tga_bytes(0x20, 2, 24, &top)).unwrap();
    let b = load_tga_from_memory(&tga_bytes(0, 2, 24, &bottom)).unwrap();
    assert_eq!((a.width, a.height, a.bits_per_pixel), (2, 2, 24));
    assert_eq!(a.format, ImageFormat::Rgba8UnormSrgb);
    assert_eq!(a.data, b.data);
    assert_eq!(
        a.data,
        [
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255
        ]
    );
}

#[test]
fn tga_rle_retains_original_alpha_channel_depth() {
    let image = load_tga_from_memory(&tga_bytes(0x28, 10, 32, &[0x83, 10, 20, 30, 40])).unwrap();
    assert_eq!(image.bits_per_pixel, 32);
    assert_eq!(image.data, [30, 20, 10, 40].repeat(4));
}

#[test]
fn malformed_payload_errors_keep_decoder_diagnostics() {
    assert!(
        matches!(load_dds_from_memory(b"FAIL"), Err(ImageDecodeError::InvalidData(message)) if message == "Invalid DDS magic number")
    );
    assert!(
        matches!(load_tga_from_memory(&[]), Err(ImageDecodeError::InvalidData(message)) if message == "TGA file too small")
    );
    for decode in [decode_dxt1, decode_dxt3, decode_dxt5] {
        assert!(
            matches!(decode(&[], 4, 4), Err(ImageDecodeError::InvalidData(message)) if message.ends_with("data truncated"))
        );
    }
}
