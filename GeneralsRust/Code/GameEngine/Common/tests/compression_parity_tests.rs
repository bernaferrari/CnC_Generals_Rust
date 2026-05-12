use game_engine::common::system::compression::{
    compress_data, decompress_data, get_preferred_compression, get_uncompressed_size,
    is_data_compressed, CompressionEngine, CompressionInterface, CompressionLevel, CompressionType,
};

#[test]
fn top_level_zlib_uses_cpp_header_and_uncompressed_size() {
    let data = b"Generals compressed chunk payload".repeat(4);
    let compressed = compress_data(&data, CompressionType::Zlib, CompressionLevel::Best)
        .expect("compress cpp zlib");

    assert_eq!(&compressed[0..4], b"ZL9\0");
    assert!(is_data_compressed(&compressed));
    assert_eq!(get_uncompressed_size(&compressed), Some(data.len()));
    assert_eq!(
        decompress_data(&compressed).expect("decompress cpp zlib"),
        data
    );
}

#[test]
fn preferred_compression_matches_cpp_refpack() {
    let data = b"Generals RefPack compressed payload".repeat(4);
    let compressed = compress_data(
        &data,
        get_preferred_compression(),
        CompressionLevel::Default,
    )
    .expect("compress cpp refpack");

    assert_eq!(get_preferred_compression(), CompressionType::RefPack);
    assert_eq!(&compressed[0..4], b"EAR\0");
    assert!(is_data_compressed(&compressed));
    assert_eq!(get_uncompressed_size(&compressed), Some(data.len()));
    assert_eq!(
        decompress_data(&compressed).expect("decompress cpp refpack"),
        data
    );
}

#[test]
fn compression_engine_uses_cpp_headers_for_zlib_and_refpack() {
    let engine = CompressionEngine::new();
    let data = b"engine compression parity payload".repeat(3);

    let zlib = engine
        .compress(&data, CompressionType::Zlib, CompressionLevel::Default)
        .expect("engine zlib");
    assert_eq!(&zlib.compressed_data[0..4], b"ZL5\0");
    assert_eq!(
        engine
            .decompress(&zlib.compressed_data, CompressionType::Zlib, None)
            .expect("engine zlib decompress"),
        data
    );

    let refpack = engine
        .compress(&data, CompressionType::RefPack, CompressionLevel::Default)
        .expect("engine refpack");
    assert_eq!(&refpack.compressed_data[0..4], b"EAR\0");
    assert_eq!(
        engine
            .decompress(&refpack.compressed_data, CompressionType::RefPack, None)
            .expect("engine refpack decompress"),
        data
    );
}

#[test]
fn lz4_is_not_reported_as_cpp_compressed_data() {
    let data = b"legacy unsupported lz4";
    let result = compress_data(data, CompressionType::LZ4, CompressionLevel::Default);
    assert!(result.is_err(), "C++ Generals compression has no LZ4 type");
}
