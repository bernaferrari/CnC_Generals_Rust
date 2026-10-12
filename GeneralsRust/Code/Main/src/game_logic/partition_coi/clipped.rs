//! Receiving-map admission for the original partition footprint sampler.
use super::{FootprintCells, FootprintSink, HostPartitionFootprint, stamp_footprint};
use crate::game_logic::partition_manager::PARTITION_CELL_SIZE_RESIDUAL;

/// Saved geometry is not trusted content. This bound applies to original
/// half-cell samples (boxes) or midpoint rows (circles), before enumeration.
/// It permits a 20,480 by 20,480 world-unit box; larger unsupported saved
/// footprints are rejected explicitly, never replaced with an approximation.
const MAX_FOOTPRINT_SAMPLES: u64 = 1 << 20;

pub(crate) fn footprint_sample_cost(fp: HostPartitionFootprint) -> Result<u64, &'static str> {
    if ![fp.major_radius, fp.minor_radius, fp.angle]
        .iter()
        .all(|value| value.is_finite())
        || fp.major_radius < 0.0
        || fp.minor_radius < 0.0
    {
        return Err("Non-finite or negative partition geometry");
    }
    let samples = if fp.is_small {
        4
    } else if fp.is_box {
        let step = PARTITION_CELL_SIZE_RESIDUAL * 0.5;
        // Keep the source f32 step-count observation before widening.
        let x = (fp.major_radius * (2.0 / step)).ceil() as u64;
        let z = (fp.minor_radius * (2.0 / step)).ceil() as u64;
        x.checked_mul(z)
            .ok_or("Partition footprint sample count overflow")?
    } else {
        (fp.major_radius / PARTITION_CELL_SIZE_RESIDUAL)
            .ceil()
            .max(1.0) as u64
    };
    if samples > MAX_FOOTPRINT_SAMPLES {
        return Err("Saved partition footprint exceeds the supported sample budget");
    }
    Ok(samples)
}

struct ClippedCells {
    dimensions: [i64; 2],
    collected: FootprintCells,
}

impl FootprintSink for ClippedCells {
    fn cell(&mut self, x: i64, z: i64) {
        if x >= 0 && z >= 0 && x < self.dimensions[0] && z < self.dimensions[1] {
            self.collected.cell(x, z);
        }
    }

    fn span(&mut self, lo: i64, hi: i64, z: i64) {
        // CPP hLineCircle checks getCellAt before admitting each COI. Clip
        // the same scanline first so a large off-map span allocates nothing.
        if z < 0 || z >= self.dimensions[1] {
            return;
        }
        for x in lo.max(0)..=hi.min(self.dimensions[0] - 1) {
            self.collected.cell(x, z);
        }
    }
}

pub(crate) fn cells_touched_for_footprint_clipped(
    origin: [f32; 2],
    dimensions: [usize; 2],
    x: f32,
    z: f32,
    fp: HostPartitionFootprint,
) -> Result<Vec<(i32, i32)>, &'static str> {
    footprint_sample_cost(fp)?;
    if ![origin[0], origin[1], x, z]
        .iter()
        .all(|value| value.is_finite())
    {
        return Err("Non-finite partition position or map origin");
    }
    let width = i32::try_from(dimensions[0]).map_err(|_| "Partition map width exceeds Int")?;
    let height = i32::try_from(dimensions[1]).map_err(|_| "Partition map height exceeds Int")?;
    let mut cells = ClippedCells {
        dimensions: [i64::from(width), i64::from(height)],
        collected: FootprintCells::default(),
    };
    stamp_footprint(origin, x, z, fp, &mut cells);
    Ok(cells.collected.cells)
}
