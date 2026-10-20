//! Original BaseHeightMap::Cast_Ray over a receiving world's borrowed bytes.
//! Inputs and outputs use original XYZ (height Z), never Main's Y-up layout.
use crate::common::{MAP_HEIGHT_SCALE, MAP_XY_FACTOR};
use math_utilities::{AABox, CastResult, CollisionMath, LineSegment, Triangle, Vector3};

/// No allocation, owner publication, or alternate height buffer.
#[derive(Clone, Copy)]
pub struct TerrainHeightSamples<'a> {
    samples: &'a [u8],
    width: i32,
    height: i32,
    border: i32,
}

pub struct TerrainRayCast {
    pub hit: bool,
    pub result: CastResult,
}

impl<'a> TerrainHeightSamples<'a> {
    pub fn new(samples: &'a [u8], width: i32, height: i32, border: i32) -> Result<Self, String> {
        let len = usize::try_from(width)
            .ok()
            .and_then(|w| usize::try_from(height).ok().and_then(|h| w.checked_mul(h)));
        // Leave room for the source overhang, expanded cells and index border.
        if width <= 0
            || height <= 0
            || border < 0
            || width
                .checked_add(border)
                .and_then(|n| n.checked_add(border))
                .and_then(|n| n.checked_add(132))
                .is_none()
            || height
                .checked_add(border)
                .and_then(|n| n.checked_add(border))
                .and_then(|n| n.checked_add(132))
                .is_none()
            || len != Some(samples.len())
        {
            return Err("invalid terrain ray byte grid dimensions/border/sample count".into());
        }
        Ok(Self {
            samples,
            width,
            height,
            border,
        })
    }

    fn clipped_height(self, x: i32, y: i32) -> i32 {
        let x = x.clamp(0, self.width - 1) as usize;
        let y = y.clamp(0, self.height - 1) as usize;
        self.samples[y * self.width as usize + x] as i32
    }

    /// The finite source Cast_Ray operation. A malformed grid/ray is an error,
    /// distinct from a valid ray that misses the receiving terrain.
    pub fn cast_ray(self, start: Vector3, end: Vector3) -> Result<TerrainRayCast, String> {
        if !start.is_finite() || !end.is_finite() || !(end - start).is_finite() {
            return Err("nonfinite terrain ray segment".into());
        }
        let ray = LineSegment::new(start, end);
        let overhang = 64 + self.border;
        let mut bounds = AABox::from_min_max(
            Vector3::new(
                -MAP_XY_FACTOR * overhang as f32,
                -MAP_XY_FACTOR * overhang as f32,
                -MAP_XY_FACTOR,
            ),
            Vector3::new(
                MAP_XY_FACTOR * (self.width + overhang) as f32,
                MAP_XY_FACTOR * (self.height + overhang) as f32,
                MAP_HEIGHT_SCALE * 255. + MAP_XY_FACTOR,
            ),
        );
        let mut p0 = start;
        let mut p1 = end;
        let mut cells = [0_i32; 4];
        for pass in 0..3 {
            let mut clipping = CastResult {
                compute_contact_point: true,
                ..Default::default()
            };
            if CollisionMath::collide_line_aabox(&ray, &bounds, &mut clipping) {
                if !clipping.start_bad {
                    p0 = clipping.contact_point;
                }
                clipping.fraction = 1.;
                clipping.start_bad = false;
                let reverse = LineSegment::new(end, start);
                if CollisionMath::collide_line_aabox(&reverse, &bounds, &mut clipping)
                    && !clipping.start_bad
                {
                    p1 = clipping.contact_point;
                }
            } else {
                if pass == 0 {
                    return Ok(TerrainRayCast {
                        hit: false,
                        result: CastResult::default(),
                    });
                }
                break;
            }
            let (x0, x1) = if p0.x > p1.x {
                (p1.x, p0.x)
            } else {
                (p0.x, p1.x)
            };
            let (y0, y1) = if p0.y > p1.y {
                (p1.y, p0.y)
            } else {
                (p0.y, p1.y)
            };
            cells = [
                source_floor(x0 / MAP_XY_FACTOR)?,
                source_ceil(x1 / MAP_XY_FACTOR)?,
                source_floor(y0 / MAP_XY_FACTOR)?,
                source_ceil(y1 / MAP_XY_FACTOR)?,
            ];
            let [sx, ex, sy, ey] = cells;
            let mut minimum = 255;
            let mut maximum = 0;
            for y in sy..=ey {
                for x in sx..=ex {
                    let value = self.clipped_height(x + self.border, y + self.border);
                    minimum = minimum.min(value);
                    maximum = maximum.max(value);
                }
            }
            bounds = AABox::from_min_max(
                Vector3::new(
                    MAP_XY_FACTOR * (sx - 1) as f32,
                    MAP_XY_FACTOR * (sy - 1) as f32,
                    MAP_HEIGHT_SCALE * (minimum - 1) as f32,
                ),
                Vector3::new(
                    MAP_XY_FACTOR * (ex + 1) as f32,
                    MAP_XY_FACTOR * (ey + 1) as f32,
                    MAP_HEIGHT_SCALE * (maximum + 1) as f32,
                ),
            );
        }
        let [sx, ex, sy, ey] = cells.map(|n| n + self.border);
        let mut result = CastResult {
            compute_contact_point: true,
            ..Default::default()
        };
        let mut hit = false;
        for offset in [1, 3] {
            for y in (sy - offset)..=(ey + offset) {
                for x in (sx - offset)..=(ex + offset) {
                    let point = |x: i32, y: i32| {
                        Vector3::new(
                            MAP_XY_FACTOR * (x - self.border) as f32,
                            MAP_XY_FACTOR * (y - self.border) as f32,
                            MAP_HEIGHT_SCALE * self.clipped_height(x, y) as f32,
                        )
                    };
                    let p0 = point(x, y);
                    let p1 = point(x + 1, y);
                    let p2 = point(x + 1, y + 1);
                    let p3 = point(x, y + 1);
                    // Preserve the source logical OR: after the first hit no
                    // later triangle collision is called, despite the comment
                    // in C++ suggesting that a closer hit might be considered.
                    let bottom = Triangle::new(p0, p1, p2);
                    hit = hit || CollisionMath::collide_line_triangle(&ray, &bottom, &mut result);
                    if result.start_bad {
                        return Ok(TerrainRayCast { hit: true, result });
                    }
                    let top = Triangle::new(p2, p3, p0);
                    hit = hit || CollisionMath::collide_line_triangle(&ray, &top, &mut result);
                    if hit {
                        result.surface_type = 13;
                    }
                }
            }
        }
        Ok(TerrainRayCast { hit, result })
    }
}

// BaseType.h's fast_float_floor/ceil subtract/add the preceding f32 of 1
// before truncation. In particular, negative exact integers need this original
// operation, not mathematical floor. Integer conversion follows finite PC24
// source inputs, with explicit rejection outside Rust's checked index domain.
fn source_integer(value: f32) -> Result<i32, String> {
    if !value.is_finite() || value <= i32::MIN as f32 + 256. || value >= i32::MAX as f32 - 256. {
        return Err("terrain ray cell coordinate exceeds checked index domain".into());
    }
    Ok(value.trunc() as i32)
}
pub(super) fn source_floor(value: f32) -> Result<i32, String> {
    source_integer(if value.is_sign_negative() {
        value - f32::from_bits(0x3f7fffff)
    } else {
        value
    })
}
fn source_ceil(value: f32) -> Result<i32, String> {
    source_integer(if !value.is_sign_negative() {
        value + f32::from_bits(0x3f7fffff)
    } else {
        value
    })
}

impl super::TerrainLogic {
    /// Cast against this instance's current terrain, in original XYZ space.
    /// An unloaded map misses; malformed loaded samples remain an error.
    pub fn cast_terrain_ray(&self, start: Vector3, end: Vector3) -> Result<TerrainRayCast, String> {
        if self.map_dx == 0 && self.map_dy == 0 && self.map_data.is_empty() {
            return Ok(TerrainRayCast {
                hit: false,
                result: CastResult::default(),
            });
        }
        TerrainHeightSamples::new(&self.map_data, self.map_dx, self.map_dy, self.border_size)?
            .cast_ray(start, end)
    }
}

#[cfg(test)]
#[path = "terrain_ray_tests.rs"]
mod tests;
