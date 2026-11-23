// Two 4x4 BIKi frames, each a single FILL block cropped from every plane.
pub(crate) fn two_frames() -> Vec<u8> {
    clip(&[frame(96), frame(110)])
}

pub(crate) fn frame(luma: u8) -> Vec<u8> {
    let mut packet = vec![0; 4]; // BIKi plane-end marker.
    for color in [luma, 128, 128] {
        let mut plane = Vec::new();
        let mut position = 0;
        let mut push = |width: usize, value: usize| {
            for bit in 0..width {
                if position % 8 == 0 {
                    plane.push(0);
                }
                plane[position / 8] |= (((value >> bit) & 1) as u8) << (position % 8);
                position += 1;
            }
        };
        for _ in 0..23 {
            push(4, 0); // Identity Huffman descriptors.
        }
        push(10, 1); // One block type.
        push(1, 1); // Repeated value.
        push(4, 6); // FILL.
        push(9, 0); // Empty scaled-block bundle at coded width 8.
        push(10, 1); // One color.
        push(1, 0); // Explicit Huffman-coded value.
        push(4, usize::from(color >> 4));
        push(4, usize::from(color & 0xf));
        for _ in 0..6 {
            push(10, 0); // Empty pattern, motion, DC and run bundles.
        }
        plane.resize(plane.len().next_multiple_of(4), 0);
        packet.extend(plane);
    }
    packet
}

pub(crate) fn clip(packets: &[Vec<u8>]) -> Vec<u8> {
    let payload = packets.iter().map(Vec::len).sum::<usize>();
    let table_end = 44 + 4 * packets.len();
    let mut bytes = vec![0; table_end];
    bytes[..4].copy_from_slice(b"BIKi");
    for (offset, value) in [
        (4, (table_end + payload - 8) as u32),
        (8, packets.len() as u32),
        (12, packets.iter().map(Vec::len).max().unwrap() as u32),
        (16, packets.len() as u32),
        (20, 4),
        (24, 4),
        (28, 30),
        (32, 1),
    ] {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    for (index, packet) in packets.iter().enumerate() {
        let entry = 44 + index * 4;
        let offset = bytes.len() as u32;
        bytes[entry..entry + 4].copy_from_slice(&offset.to_le_bytes());
        bytes.extend(packet);
    }
    bytes
}
