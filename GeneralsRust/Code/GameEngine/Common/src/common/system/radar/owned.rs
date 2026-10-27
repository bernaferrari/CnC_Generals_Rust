//! Match-owned radar phases and admission of explicit terrain samples.
use super::*;

impl RadarSystem {
    /// Admit a synchronous logic event with this session's frame.
    pub fn try_event_owned(
        &mut self,
        frame: u32,
        kind: RadarEventType,
        position: &Coord3D,
    ) -> bool {
        self.current_frame = frame;
        self.try_event(kind, position)
    }

    pub fn contains_object(&self, id: u32) -> bool {
        self.local_object_list
            .iter()
            .chain(self.object_list.iter())
            .any(|object| object.object_id == id)
    }

    /// Advance the supplied match radar without selecting process-wide stores.
    /// Returns whether its owner must re-sample terrain at the original phase.
    pub fn update_owned(&mut self, current_frame: u32) -> bool {
        self.current_frame = current_frame;
        let mut refresh = false;
        // Update events - check if any should die
        for event in &mut self.events {
            if event.active && event.create_frame > 0 && current_frame > event.die_frame {
                event.active = false;
            }
        }

        // Check for queued terrain refresh
        if let Some(refresh_frame) = self.queue_terrain_refresh_frame {
            if current_frame.wrapping_sub(refresh_frame) > RADAR_QUEUE_TERRAIN_REFRESH_DELAY {
                self.queue_terrain_refresh_frame = None;
                refresh = true;
            }
        }

        // Update GPS satellite (deactivate if expired)
        if self.is_gps_active() && current_frame >= self.gps_active_until_frame {
            self.deactivate_gps_satellite();
        }

        // Update radar scans (remove expired ones)
        self.update_radar_scans();

        // Update jamming status for all objects
        self.update_jamming_status();

        // Update stealth detection
        self.update_stealth_detection();
        refresh
    }

    /// Repaint already admitted samples without selecting a map provider.
    pub fn refresh_owned_terrain(&mut self) {
        self.queue_terrain_refresh_frame = None;
        self.build_terrain_texture_with_source(None, self.water_radar_color);
    }

    pub fn refresh_owned_terrain_with_source(
        &mut self,
        source: &dyn RadarTerrainPaintSource,
        water: [f32; 3],
    ) {
        self.queue_terrain_refresh_frame = None;
        self.water_radar_color = water;
        self.build_terrain_texture_with_source(Some(source), water);
    }

    /// Observe the driving clock without running another update phase.
    pub fn observe_frame(&mut self, frame: u32) {
        self.current_frame = frame;
    }

    /// Admit the driving match samples without a global map/snapshot callback.
    pub fn new_map_owned(
        &mut self,
        map_min: Coord3D,
        map_max: Coord3D,
        terrain_heights: &[(f32, f32, bool)],
    ) {
        self.new_map_impl(map_min, map_max, terrain_heights, true);
    }

    pub(super) fn new_map_impl(
        &mut self,
        map_min: Coord3D,
        map_max: Coord3D,
        terrain_heights: &[(f32, f32, bool)],
        owned: bool,
    ) {
        // C++ Radar::newMap resets before admitting the supplied terrain.

        self.reset();

        self.map_extent = Region3D {
            lo: map_min,
            hi: map_max,
        };

        // Calculate sampling intervals
        self.x_sample = self.map_extent.width() / RADAR_CELL_WIDTH as f32;
        self.y_sample = self.map_extent.height() / RADAR_CELL_HEIGHT as f32;

        // C++ averages every other radar cell (`y+=2`, `x+=2`).
        let mut terrain_sum = 0.0;
        let mut water_sum = 0.0;
        let mut terrain_count = 0;
        let mut water_count = 0;
        let expected_samples = (RADAR_CELL_WIDTH * RADAR_CELL_HEIGHT) as usize;
        let full_grid = terrain_heights.len() == expected_samples;

        if full_grid {
            for y in (0..RADAR_CELL_HEIGHT).step_by(2) {
                for x in (0..RADAR_CELL_WIDTH).step_by(2) {
                    let (_, z, is_water) = terrain_heights[(y * RADAR_CELL_WIDTH + x) as usize];
                    if is_water {
                        water_sum += z;
                        water_count += 1;
                    } else {
                        terrain_sum += z;
                        terrain_count += 1;
                    }
                }
            }
        } else {
            for &(_x, z, is_water) in terrain_heights {
                if is_water {
                    water_sum += z;
                    water_count += 1;
                } else {
                    terrain_sum += z;
                    terrain_count += 1;
                }
            }
        }

        self.terrain_average_z = if terrain_count > 0 {
            terrain_sum / terrain_count as f32
        } else {
            0.0
        };

        self.water_average_z = if water_count > 0 {
            water_sum / water_count as f32
        } else {
            0.0
        };

        self.terrain_samples.clear();
        if full_grid {
            self.terrain_samples.reserve(expected_samples);
            for &(_x, z, is_water) in terrain_heights {
                self.terrain_samples.push(RadarTerrainSample {
                    height: z,
                    is_water,
                });
            }
        }

        self.terrain_dirty = true;
        if owned {
            self.refresh_owned_terrain();
        } else {
            self.refresh_terrain();
            snapshot::ensure_the_radar_snapshot_block();
        }
    }
}
