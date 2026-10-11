//! Immutable completed radar input for rendering and input translation.
//! The event ring, timers, and soundPlayed admission remain on the session owner.
use super::*;

#[derive(Debug, Clone)]
pub struct RadarPresentation {
    identity: Arc<()>,
    event_keys: Vec<(usize, Arc<()>, bool, bool)>,
    map_extent: Region3D,
    x_sample: f32,
    y_sample: f32,
    terrain_average_z: f32,
    terrain_samples: Vec<RadarTerrainSample>,
    local_object_list: Vec<RadarObject>,
    current_frame: u32,
    terrain_generation: u64,
    shroud_revision: u64,
    radar_forced: bool,
    radar_hidden: bool,
    local_has_radar: bool,
    last_event: Option<Coord3D>,
    events: Vec<RadarEvent>,
    terrain: Vec<u8>,
    overlay: Vec<u8>,
    shroud: Vec<u8>,
}

impl RadarSystem {
    pub fn presentation(&self) -> RadarPresentation {
        RadarPresentation {
            identity: self.owner_identity.clone(),
            event_keys: self
                .events
                .iter()
                .enumerate()
                .filter(|(_, event)| event.active && event.event_type != RadarEventType::Fake)
                .map(|(index, event)| {
                    (
                        index,
                        self.event_identities[index].clone(),
                        event.sound_played,
                        event.event_type == RadarEventType::BeaconPulse,
                    )
                })
                .collect(),
            map_extent: self.map_extent,
            x_sample: self.x_sample,
            y_sample: self.y_sample,
            terrain_average_z: self.terrain_average_z,
            current_frame: self.current_frame,
            terrain_generation: self.terrain_generation,
            shroud_revision: self.shroud_revision,
            radar_forced: self.is_radar_forced(),
            radar_hidden: self.radar_hidden,
            local_has_radar: self.local_has_radar,
            terrain_samples: self.terrain_samples.clone(),
            local_object_list: self.local_object_list.clone(),
            last_event: self.get_last_event_loc(),
            events: self.drawable_events().into_iter().cloned().collect(),
            terrain: self.get_terrain_texture().to_vec(),
            overlay: self.build_object_overlay_texture_rgba(),
            shroud: self.build_shroud_texture_rgba(),
        }
    }
}
impl RadarSystem {
    pub fn acknowledge_presented_events(
        &mut self,
        identity: &Arc<()>,
        events: &[(usize, Arc<()>)],
    ) {
        if !Arc::ptr_eq(identity, &self.owner_identity) {
            return;
        }
        for (index, identity) in events {
            if let Some(event) = self.events.get_mut(*index) {
                if event.active && Arc::ptr_eq(identity, &self.event_identities[*index]) {
                    event.sound_played = true;
                }
            }
        }
    }
}
impl RadarPresentation {
    pub fn identity(&self) -> &Arc<()> {
        &self.identity
    }
    pub fn event_keys(&self) -> &[(usize, Arc<()>, bool, bool)] {
        &self.event_keys
    }

    pub fn is_radar_shown(&self) -> bool {
        self.radar_forced || (!self.radar_hidden && self.local_has_radar)
    }
    pub fn is_radar_forced(&self) -> bool {
        self.radar_forced
    }
    pub fn is_radar_hidden(&self) -> bool {
        self.radar_hidden
    }
    pub fn local_has_radar(&self) -> bool {
        self.local_has_radar
    }
    pub fn get_last_event_loc(&self) -> Option<Coord3D> {
        self.last_event
    }
    pub fn draw_events(&self) -> &[RadarEvent] {
        &self.events
    }
    pub fn get_terrain_texture(&self) -> &[u8] {
        &self.terrain
    }
    pub fn build_object_overlay_texture_rgba(&self) -> &[u8] {
        &self.overlay
    }
    pub fn build_shroud_texture_rgba(&self) -> &[u8] {
        &self.shroud
    }
    pub fn current_frame(&self) -> u32 {
        self.current_frame
    }
    pub fn map_extent(&self) -> Region3D {
        self.map_extent
    }
    pub fn terrain_average_z(&self) -> f32 {
        self.terrain_average_z
    }
    pub fn terrain_generation(&self) -> u64 {
        self.terrain_generation
    }
    pub fn shroud_revision(&self) -> u64 {
        self.shroud_revision
    }
    fn terrain_height_for_cell(&self, x: i32, y: i32) -> f32 {
        if self.terrain_samples.len() == (RADAR_CELL_WIDTH * RADAR_CELL_HEIGHT) as usize {
            let ux = x.clamp(0, (RADAR_CELL_WIDTH - 1) as i32) as u32;
            let uy = y.clamp(0, (RADAR_CELL_HEIGHT - 1) as i32) as u32;
            let idx = (uy * RADAR_CELL_WIDTH + ux) as usize;
            return self.terrain_samples[idx].height;
        }
        self.terrain_average_z
    }
    fn world_to_radar_unclamped(&self, world: &Coord3D) -> Option<ICoord2D> {
        if self.x_sample <= f32::EPSILON || self.y_sample <= f32::EPSILON {
            return None;
        }

        Some(ICoord2D {
            x: ((world.x - self.map_extent.lo.x) / self.x_sample) as i32,
            y: ((world.y - self.map_extent.lo.y) / self.y_sample) as i32,
        })
    }
    pub fn radar_to_world(&self, radar: &ICoord2D) -> Option<Coord3D> {
        let x = radar.x.clamp(0, (RADAR_CELL_WIDTH - 1) as i32);
        let y = radar.y.clamp(0, (RADAR_CELL_HEIGHT - 1) as i32);

        Some(Coord3D {
            x: self.map_extent.lo.x + x as f32 * self.x_sample,
            y: self.map_extent.lo.y + y as f32 * self.y_sample,
            z: self.terrain_height_for_cell(x, y),
        })
    }
    pub fn world_to_radar(&self, world: &Coord3D) -> Option<ICoord2D> {
        if self.x_sample <= f32::EPSILON || self.y_sample <= f32::EPSILON {
            return None;
        }

        let mut x = ((world.x - self.map_extent.lo.x) / self.x_sample) as i32;
        let mut y = ((world.y - self.map_extent.lo.y) / self.y_sample) as i32;

        // Clamp to radar bounds
        x = x.clamp(0, (RADAR_CELL_WIDTH - 1) as i32);
        y = y.clamp(0, (RADAR_CELL_HEIGHT - 1) as i32);

        Some(ICoord2D { x, y })
    }
    pub fn build_hero_reticle_object_ids(&self) -> Vec<u32> {
        self.local_object_list
            .iter()
            .filter(|obj| obj.is_hero && !obj.is_temporarily_hidden())
            .map(|obj| obj.object_id)
            .collect()
    }
    pub fn build_hero_reticle_rects_for_objects(
        &self,
        object_ids: &[u32],
        pixel_x: i32,
        pixel_y: i32,
        width: i32,
        height: i32,
        icon_width: i32,
        icon_height: i32,
    ) -> Vec<RadarHeroReticleRect> {
        object_ids
            .iter()
            .filter_map(|object_id| {
                self.local_object_list
                    .iter()
                    .find(|obj| obj.object_id == *object_id)
            })
            .filter_map(|obj| {
                let radar_point = self.world_to_radar(&obj.world_pos)?;
                let offset_screen = radar_to_pixel(&radar_point, pixel_x, pixel_y, width, height);
                let x1 = offset_screen.x - (icon_width / 2) + 1;
                let y1 = offset_screen.y - (icon_height / 2);
                Some(RadarHeroReticleRect {
                    x1,
                    y1,
                    x2: x1 + icon_width,
                    y2: y1 + icon_height,
                })
            })
            .collect()
    }
    pub fn build_view_box_lines(
        &self,
        origin_world: Coord3D,
        corner_world: [Coord3D; 4],
        pixel_x: i32,
        pixel_y: i32,
        width: i32,
        height: i32,
        clip_x: i32,
        clip_y: i32,
        clip_width: i32,
        clip_height: i32,
    ) -> Vec<RadarViewBoxLine> {
        let Some(ul_radar) = self.world_to_radar_unclamped(&origin_world) else {
            return Vec::new();
        };

        let mut corners = [ICoord2D::new(0, 0); 4];
        for (index, world) in corner_world.iter().enumerate() {
            let Some(radar) = self.world_to_radar_unclamped(world) else {
                return Vec::new();
            };
            corners[index] = radar;
        }

        let mut view_box_offsets = [ICoord2D::new(0, 0); 4];
        for index in 1..4 {
            view_box_offsets[index] = ICoord2D::new(
                corners[index].x - corners[index - 1].x,
                corners[index].y - corners[index - 1].y,
            );
        }

        let top_color = radar_view_box_top_color();
        let bottom_color = radar_view_box_bottom_color();
        let mut lines = Vec::with_capacity(4);
        let mut start = radar_to_pixel(&ul_radar, pixel_x, pixel_y, width, height);
        let mut radar = ICoord2D::new(
            ul_radar.x + view_box_offsets[1].x,
            ul_radar.y + view_box_offsets[1].y,
        );
        let mut end = radar_to_pixel(&radar, pixel_x, pixel_y, width, height);

        let mut push_line =
            |start: ICoord2D, end: ICoord2D, start_color: RGBAColorInt, end_color: RGBAColorInt| {
                if let Some((start, end)) =
                    clip_line_to_rect(start, end, clip_x, clip_y, clip_width, clip_height)
                {
                    lines.push(RadarViewBoxLine {
                        start,
                        end,
                        start_color,
                        end_color,
                    });
                }
            };

        push_line(start, end, top_color, top_color);

        start = end;
        radar.x += view_box_offsets[2].x;
        radar.y += view_box_offsets[2].y;
        end = radar_to_pixel(&radar, pixel_x, pixel_y, width, height);
        push_line(start, end, top_color, bottom_color);

        start = end;
        radar.x += view_box_offsets[3].x;
        radar.y += view_box_offsets[3].y;
        end = radar_to_pixel(&radar, pixel_x, pixel_y, width, height);
        push_line(start, end, bottom_color, bottom_color);

        start = end;
        end = radar_to_pixel(&ul_radar, pixel_x, pixel_y, width, height);
        push_line(start, end, bottom_color, top_color);

        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radar_presentation_is_frozen_and_foreign_acknowledgments_are_ignored() {
        let mut a = RadarSystem::new();
        let mut b = RadarSystem::new();
        let first = Coord3D::new(0.25, 0.5, 0.0);
        a.create_event(&first, RadarEventType::Information, 1.0);
        b.create_event(&first, RadarEventType::Information, 1.0);
        let frozen = a.presentation();
        let key = frozen.event_keys()[0].clone();
        b.acknowledge_presented_events(frozen.identity(), &[(key.0, key.1.clone())]);
        assert!(!b.events[0].sound_played);
        a.hide(true);
        assert!(!frozen.is_radar_hidden());
        a.acknowledge_presented_events(frozen.identity(), &[(key.0, key.1)]);
        assert!(a.events[0].sound_played);
        assert!(!frozen.draw_events()[0].sound_played);
    }

    #[test]
    fn radar_presentation_stale_ack_does_not_mark_same_frame_reused_slot() {
        let mut radar = RadarSystem::new();
        let position = Coord3D::new(0.5, 0.5, 0.0);
        radar.create_event(&position, RadarEventType::Information, 1.0);
        let old = radar.presentation();
        for _ in 0..MAX_RADAR_EVENTS {
            radar.create_event(&position, RadarEventType::Information, 1.0);
        }
        let key = old.event_keys()[0].clone();
        radar.acknowledge_presented_events(old.identity(), &[(key.0, key.1)]);
        assert!(!radar.events[0].sound_played);
    }

    #[test]
    fn radar_presentation_geometry_and_layers_match_owner() {
        let radar = RadarSystem::new();
        let frozen = radar.presentation();
        let cell = ICoord2D::new(20, 40);
        let world = radar.radar_to_world(&cell).unwrap();
        let actual = frozen.radar_to_world(&cell).unwrap();
        assert_eq!((world.x, world.y, world.z), (actual.x, actual.y, actual.z));
        assert_eq!(radar.world_to_radar(&world), frozen.world_to_radar(&world));
        assert_eq!(radar.get_terrain_texture(), frozen.get_terrain_texture());
        assert_eq!(
            radar.build_object_overlay_texture_rgba(),
            frozen.build_object_overlay_texture_rgba()
        );
        assert_eq!(
            radar.build_shroud_texture_rgba(),
            frozen.build_shroud_texture_rgba()
        );
    }
}
