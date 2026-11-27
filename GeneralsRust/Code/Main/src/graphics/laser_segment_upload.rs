//! Interleaved line vertices for the renderer's additive laser pass.

use crate::game_logic::host_base_defense::HostLaserLine3DSegment;
use crate::presentation_frame::{PresentationFrame, PresentationLaserBeam};

/// Floats per packed laser vertex (pos.xyz + uv.xy + color.rgba = 9 × f32).
pub const LASER_VERTEX_FLOATS: usize = 9;
/// Two vertices (start + end) per Line3D segment.
pub const LASER_VERTS_PER_SEGMENT: usize = 2;
/// Bytes per Line3D segment in the upload buffer.
pub const LASER_BYTES_PER_SEGMENT: usize =
    LASER_VERTEX_FLOATS * LASER_VERTS_PER_SEGMENT * std::mem::size_of::<f32>();

/// One interleaved line vertex.
#[derive(Debug, Clone, Copy, PartialEq)]
struct LaserSegmentVertex {
    pub position: [f32; 3],
    /// U = tile factor along beam, V = scroll offset.
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

impl LaserSegmentVertex {
    fn to_floats(self) -> [f32; LASER_VERTEX_FLOATS] {
        [
            self.position[0],
            self.position[1],
            self.position[2],
            self.uv[0],
            self.uv[1],
            self.color[0],
            self.color[1],
            self.color[2],
            self.color[3],
        ]
    }
}

/// Packed line vertices ready for `Queue::write_buffer`.
#[derive(Debug, Clone, PartialEq)]
pub struct LaserSegmentUpload {
    pub vertex_bytes: Vec<u8>,
}

impl LaserSegmentUpload {
    pub fn empty() -> Self {
        Self {
            vertex_bytes: Vec::new(),
        }
    }

    pub fn vertex_count(&self) -> u32 {
        let count = self.vertex_bytes.len() / (LASER_BYTES_PER_SEGMENT / LASER_VERTS_PER_SEGMENT);
        u32::try_from(count).unwrap_or(u32::MAX)
    }

    /// Pack all beams from a presentation snapshot (including projectile trails and client scene lines).
    pub fn pack_from_presentation(frame: &PresentationFrame) -> Self {
        Self::pack_beams_streams_and_scene_lines(
            &frame.laser_beams,
            &frame.projectile_streams,
            &frame.scene_lines,
        )
    }

    /// Pack host lasers, projectile streams, and client scene lines (tracer/rope/W3DLaser).
    pub fn pack_beams_streams_and_scene_lines(
        beams: &[PresentationLaserBeam],
        streams: &[crate::presentation_frame::PresentationProjectileStream],
        scene_lines: &[crate::presentation_frame::PresentationSceneLine],
    ) -> Self {
        if beams.is_empty() && streams.iter().all(|s| s.points.len() < 2) && scene_lines.is_empty()
        {
            return Self::empty();
        }
        let mut floats = Vec::new();
        for beam in beams {
            for seg in &beam.segments {
                let host = HostLaserLine3DSegment {
                    start: seg.start,
                    end: seg.end,
                    width: seg.width,
                    tile_factor: seg.tile_factor,
                    scroll_offset: seg.scroll_offset,
                };
                let color = beam.inner_color;
                let verts = segment_to_vertices(&host, color);
                for v in verts {
                    floats.extend_from_slice(&v.to_floats());
                }
            }
        }
        for stream in streams {
            if stream.points.len() < 2 {
                continue;
            }
            let (color, width) = projectile_stream_draw_style(&stream.stream_name);
            for i in 0..stream.points.len() - 1 {
                let start = stream.points[i];
                let end = stream.points[i + 1];
                let start_v = glam::Vec3::new(start.0, start.1, start.2);
                let end_v = glam::Vec3::new(end.0, end.1, end.2);
                if crate::game_logic::host_projectile_stream::is_stream_hole(start_v)
                    || crate::game_logic::host_projectile_stream::is_stream_hole(end_v)
                {
                    continue;
                }
                let host = HostLaserLine3DSegment {
                    start,
                    end,
                    width,
                    tile_factor: 1.0,
                    scroll_offset: (i as f32) * 0.05,
                };
                let verts = segment_to_vertices(&host, color);
                for v in verts {
                    floats.extend_from_slice(&v.to_floats());
                }
            }
        }
        for line in scene_lines {
            let host = HostLaserLine3DSegment {
                start: line.start,
                end: line.end,
                width: line.width,
                tile_factor: line.tile_factor.max(0.001),
                scroll_offset: line.scroll_rate,
            };
            let verts = segment_to_vertices(&host, line.color);
            for v in verts {
                floats.extend_from_slice(&v.to_floats());
            }
        }
        Self {
            vertex_bytes: f32_slice_to_bytes(&floats),
        }
    }
}

/// Color and width for projectile stream trails.
fn projectile_stream_draw_style(stream_name: &str) -> ((f32, f32, f32, f32), f32) {
    let n = stream_name.to_ascii_lowercase();
    if n.contains("flame") || n.contains("dragon") {
        return ((1.0, 0.45, 0.08, 0.9), 2.5);
    }
    if n.contains("toxin") {
        return ((0.35, 0.9, 0.2, 0.85), 2.0);
    }
    if n.contains("microwave") {
        return ((0.6, 0.2, 0.95, 0.8), 1.5);
    }
    if n.contains("cleanup") || n.contains("hazard") {
        return ((0.4, 0.7, 1.0, 0.75), 1.8);
    }
    ((0.9, 0.9, 0.9, 0.7), 1.2)
}

fn segment_to_vertices(
    seg: &HostLaserLine3DSegment,
    color: (f32, f32, f32, f32),
) -> [LaserSegmentVertex; 2] {
    // U spans the tile factor along the beam. V carries the scroll offset.
    let c = [color.0, color.1, color.2, color.3];
    let scroll = seg.scroll_offset + 0.0;
    [
        LaserSegmentVertex {
            position: [seg.start.0, seg.start.1, seg.start.2],
            uv: [0.0, scroll],
            color: c,
        },
        LaserSegmentVertex {
            position: [seg.end.0, seg.end.1, seg.end.2],
            uv: [seg.tile_factor.max(0.0), scroll],
            color: c,
        },
    ]
}

fn f32_slice_to_bytes(floats: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(floats.len() * 4);
    for f in floats {
        out.extend_from_slice(&f.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::ObjectId;
    use crate::game_logic::host_base_defense::{
        PATRIOT_LASER_SEGMENTS, make_patriot_assist_lasers,
    };

    #[test]
    fn cpp_scene_line_freezes_height_once_and_reaches_actual_vertex_upload() {
        let line = crate::presentation_frame::PresentationSceneLine::from_cpp_desc(
            game_engine::common::system::SceneLineDesc {
                start: game_engine::common::system::geometry::Coord3D::new(13.0, 29.0, 47.0),
                end: game_engine::common::system::geometry::Coord3D::new(17.0, 31.0, 53.0),
                width: 2.0,
                color_r: 0.25,
                color_g: 0.5,
                color_b: 0.75,
                opacity: 0.8,
                texture_name: Some("OwnedLaser.tga".into()),
                tile_factor: 3.0,
                visible: true,
                scroll_rate: 0.25,
            },
        )
        .unwrap();
        assert_eq!(line.start, (13.0, 47.0, 29.0));
        assert_eq!(line.end, (17.0, 53.0, 31.0));
        let upload = LaserSegmentUpload::pack_beams_streams_and_scene_lines(&[], &[], &[line]);
        let position = (0..3)
            .map(|axis| {
                f32::from_le_bytes(
                    upload.vertex_bytes[axis * 4..axis * 4 + 4]
                        .try_into()
                        .unwrap(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(position, [13.0, 47.0, 29.0]);
        assert_eq!(upload.vertex_count(), 2);
    }

    #[test]
    fn empty_frame_upload_contains_no_vertices() {
        let pack = LaserSegmentUpload::empty();
        assert!(pack.vertex_bytes.is_empty());
        assert_eq!(pack.vertex_count(), 0);
    }

    #[test]
    fn packs_projectile_stream_trail_segments() {
        use crate::presentation_frame::PresentationProjectileStream;
        let streams = vec![PresentationProjectileStream {
            shooter_id: ObjectId(1),
            stream_name: "DragonTankFlameStream".into(),
            points: vec![(1.0, 0.0, 0.0), (10.0, 0.0, 0.0), (20.0, 0.0, 5.0)],
            target_id: None,
        }];
        let pack = LaserSegmentUpload::pack_beams_streams_and_scene_lines(&[], &streams, &[]);
        assert_eq!(pack.vertex_count(), 4);
        assert!(!pack.vertex_bytes.is_empty());
    }

    #[test]
    fn stream_hole_breaks_ribbon_segments() {
        use crate::presentation_frame::PresentationProjectileStream;
        let streams = vec![PresentationProjectileStream {
            shooter_id: ObjectId(1),
            stream_name: "DragonTankFlameStream".into(),
            points: vec![
                (1.0, 0.0, 0.0),
                (2.0, 0.0, 0.0),
                (0.0, 0.0, 0.0),
                (8.0, 0.0, 0.0),
                (9.0, 0.0, 0.0),
            ],
            target_id: None,
        }];
        let pack = LaserSegmentUpload::pack_beams_streams_and_scene_lines(&[], &streams, &[]);
        // Two live spans (1-2 and 8-9); hole-adjacent segments are skipped.
        assert_eq!(pack.vertex_count(), 4);
    }
    #[test]
    fn packs_line3d_segments_with_expected_byte_layout() {
        let beam = PresentationLaserBeam::from_host_laser(
            &make_patriot_assist_lasers(
                ObjectId(1),
                ObjectId(2),
                ObjectId(3),
                (0.0, 0.0, 0.0),
                (50.0, 0.0, 0.0),
                (100.0, 0.0, 0.0),
                0,
            )[0],
            0,
            0.0,
        );
        let pack = LaserSegmentUpload::pack_beams_streams_and_scene_lines(&[beam], &[], &[]);
        assert_eq!(
            pack.vertex_count(),
            PATRIOT_LASER_SEGMENTS * LASER_VERTS_PER_SEGMENT as u32
        );
        assert_eq!(
            pack.vertex_bytes.len(),
            (PATRIOT_LASER_SEGMENTS as usize) * LASER_BYTES_PER_SEGMENT
        );
        // First vertex position should be near start (ground skim may raise Z).
        let x0 = f32::from_le_bytes(pack.vertex_bytes[0..4].try_into().unwrap());
        assert!(x0.abs() < 1.0, "start X residual: {x0}");
    }

    #[test]
    fn packs_presentation_beams_from_assist_lasers() {
        let beams_host = make_patriot_assist_lasers(
            ObjectId(1),
            ObjectId(2),
            ObjectId(3),
            (0.0, 0.0, 5.0),
            (50.0, 0.0, 5.0),
            (100.0, 0.0, 5.0),
            0,
        );
        let beams: Vec<PresentationLaserBeam> = beams_host
            .iter()
            .enumerate()
            .map(|(i, l)| PresentationLaserBeam::from_host_laser(l, i as u32, 0.0))
            .collect();
        assert_eq!(beams.len(), 2);
        assert_eq!(beams[0].segments.len(), PATRIOT_LASER_SEGMENTS as usize);
        let pack = LaserSegmentUpload::pack_beams_streams_and_scene_lines(&beams, &[], &[]);
        assert_eq!(pack.vertex_count(), PATRIOT_LASER_SEGMENTS * 4);
    }
}
