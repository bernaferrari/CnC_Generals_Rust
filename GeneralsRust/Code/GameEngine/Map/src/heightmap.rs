//! HeightMapData chunk layouts from WorldHeightMap.cpp.

use crate::{BinaryReader, ChunkyMap, DecodeError, DecodeResult, find_chunk_by_label};

/// Decoded heightmap data extracted from the `HeightMapData` chunk.
#[derive(Debug, Clone)]
pub struct HeightMapData {
    pub width: i32,
    pub height: i32,
    pub border_size: i32,
    pub boundaries: Vec<(i32, i32)>,
    /// Raw 8-bit height samples in row-major order (size = width * height).
    pub data: Vec<u8>,
}

fn heightmap_reader(chunky: &ChunkyMap) -> DecodeResult<Option<(u16, BinaryReader<'_>)>> {
    let body = &chunky.bytes[chunky.body_offset..];
    Ok(find_chunk_by_label(body, &chunky.toc, "HeightMapData")?
        .map(|(version, payload)| (version, BinaryReader::new(payload))))
}

fn read_dimensions(version: u16, reader: &mut BinaryReader<'_>) -> DecodeResult<(i32, i32, i32)> {
    let width = reader.read_i32()?;
    let height = reader.read_i32()?;
    let border_size = if version >= 3 { reader.read_i32()? } else { 0 };
    Ok((width, height, border_size))
}

/// Read the authored dimensions. Version-one resampling applies only to full decoding.
pub fn read_heightmap_dimensions(chunky: &ChunkyMap) -> DecodeResult<Option<(i32, i32, i32)>> {
    let Some((version, mut reader)) = heightmap_reader(chunky)? else {
        return Ok(None);
    };
    Ok(Some(read_dimensions(version, &mut reader)?))
}

/// Decode height samples with the native world boundary defaults.
pub fn decode_heightmap(chunky: &ChunkyMap) -> DecodeResult<Option<HeightMapData>> {
    let Some((version, mut reader)) = heightmap_reader(chunky)? else {
        return Ok(None);
    };
    let (width, height, border_size) = read_dimensions(version, &mut reader)?;

    let boundaries = if version >= 4 {
        let count = reader.read_i32()?.max(0) as usize;
        let mut boundaries = Vec::with_capacity(count.max(1));
        for _ in 0..count {
            boundaries.push((reader.read_i32()?, reader.read_i32()?));
        }
        boundaries
    } else {
        vec![(width - 2 * border_size, height - 2 * border_size)]
    };

    let data_size = reader.read_i32()?;
    let expected = width.saturating_mul(height);
    if data_size <= 0 || data_size != expected {
        return Err(DecodeError::new(format!(
            "HeightMapData has invalid dataSize={}, expected {}",
            data_size, expected
        )));
    }

    let samples = reader.read_bytes(data_size as usize)?;
    let (width, height, data) = if version == 1 {
        downsample_version_one(width, height, samples)
    } else {
        (width, height, samples.to_vec())
    };
    Ok(Some(HeightMapData {
        width,
        height,
        border_size,
        boundaries,
        data,
    }))
}

/// Keep the even-numbered rows and columns in version-one terrain.
pub fn downsample_version_one(width: i32, height: i32, samples: &[u8]) -> (i32, i32, Vec<u8>) {
    let new_width = (width + 1) / 2;
    let new_height = (height + 1) / 2;
    let mut resized = vec![0; (new_width * new_height).max(0) as usize];
    for y in 0..new_height {
        for x in 0..new_width {
            let source_index = (2 * y * width + 2 * x) as usize;
            let destination_index = (y * new_width + x) as usize;
            if let Some(&sample) = samples.get(source_index) {
                resized[destination_index] = sample;
            }
        }
    }
    (new_width, new_height, resized)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn heightmap_chunk(version: u16, fields: &[i32], samples: &[u8]) -> ChunkyMap {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&version.to_le_bytes());
        bytes.extend_from_slice(&((fields.len() * 4 + samples.len()) as u32).to_le_bytes());
        for field in fields {
            bytes.extend_from_slice(&field.to_le_bytes());
        }
        bytes.extend_from_slice(samples);
        ChunkyMap {
            source: "HeightFixture.map".into(),
            toc: HashMap::from([(1, "HeightMapData".to_owned())]),
            body_offset: 0,
            bytes,
        }
    }

    #[test]
    fn version_one_keeps_authored_extent_and_resamples_the_height_grid() {
        let map = heightmap_chunk(1, &[5, 3, 15], &(0u8..15).collect::<Vec<_>>());
        assert_eq!(read_heightmap_dimensions(&map).unwrap(), Some((5, 3, 0)));
        let heightmap = decode_heightmap(&map).unwrap().unwrap();
        assert_eq!((heightmap.width, heightmap.height), (3, 2));
        assert_eq!(heightmap.boundaries, [(5, 3)]);
        assert_eq!(heightmap.data, [0, 2, 4, 10, 12, 14]);
    }

    #[test]
    fn version_four_keeps_boundary_order_and_reports_a_truncated_payload() {
        let fields = [3, 2, 1, 3, 5, 6, 9, 3, 5, 6, 6];
        let map = heightmap_chunk(4, &fields, &[1, 4, 9, 16, 25, 36]);
        assert_eq!(read_heightmap_dimensions(&map).unwrap(), Some((3, 2, 1)));
        let heightmap = decode_heightmap(&map).unwrap().unwrap();
        assert_eq!(heightmap.boundaries, [(5, 6), (9, 3), (5, 6)]);
        assert_eq!(heightmap.data, [1, 4, 9, 16, 25, 36]);
        let truncated = heightmap_chunk(4, &fields, &[1, 4, 9, 16, 25]);
        assert_eq!(
            decode_heightmap(&truncated).unwrap_err().to_string(),
            "Unexpected end of chunk data"
        );
    }
}
