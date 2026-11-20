//! Draw ownership of DynamicShroudClearingRangeUpdate's authored 30 grid decals.
use crate::game_logic::{GameLogic, ObjectId, ScriptPresentationEffects};
use game_client::radius_decal::{ShadowHandle, ShadowTypeInfo};
use gamelogic::object::behavior::dynamic_shroud_clearing_range_update::DynamicShroudClearingRangeUpdateModuleData;
use std::sync::{Arc, Weak};

const MARK_COUNT: usize = 30;
const GRID_SPACING: i32 = 23;
const DECAL_WIDTH: f32 = 200.0;

#[derive(Debug, Default)]
pub(crate) struct SpyGridRoster(Vec<SpyGrid>);

impl Clone for SpyGridRoster {
    fn clone(&self) -> Self {
        // DynamicShroudClearingRangeUpdate::xfer transfers creation state,
        // and loadPostProcess never recreates the receiving decal handles.
        Self::default()
    }
}

#[derive(Debug)]
struct SpyGrid {
    ping: ObjectId,
    identity: Weak<()>,
    start_frame: u32,
    grow_delay: u32,
    grow_time: u32,
    native_range: f32,
    current_range: f32,
    last_frame: Option<u32>,
    info: ShadowTypeInfo,
    color: u32,
    marks: [Option<ShadowHandle>; MARK_COUNT],
}

impl Drop for SpyGrid {
    fn drop(&mut self) {
        for mark in self.marks.iter().flatten() {
            mark.release();
        }
    }
}

impl GameLogic {
    pub(crate) fn admit_spy_grid(&mut self, ping: ObjectId) {
        let Some(ping_object) = self.objects.get(&ping) else {
            return;
        };
        let Some(template) = self
            .receiving_module_content
            .as_ref()
            .and_then(|content| content.admitted_template(&ping_object.template_name))
        else {
            return;
        };
        let Some(module) = template
            .behavior_modules()
            .iter()
            .find(|module| module.name.as_str() == "DynamicShroudClearingRangeUpdate")
        else {
            return;
        };
        let data = module.data.read();
        let Some(module) = data
            .as_any()
            .downcast_ref::<DynamicShroudClearingRangeUpdateModuleData>()
        else {
            return;
        };
        let decal = &module.grid_decal_template;
        if decal.texture_name.is_empty() {
            return;
        }
        let players = self.radius_decal_player_view(ping_object.owner_player_id);
        let Some(owner) = players.owner_player_index else {
            return;
        };
        if decal.only_visible_to_owning_player && players.local_player_index != Some(owner) {
            return;
        }
        let color = if decal.color == 0 {
            players.owner_argb
        } else {
            decal.color
        };
        self.spy_satellites.grid_decals.0.push(SpyGrid {
            ping,
            identity: Arc::downgrade(&ping_object.visual_draw_identity_token),
            start_frame: self.frame,
            grow_delay: module.grow_delay,
            grow_time: module.grow_time,
            native_range: ping_object.get_shroud_clearing_range(),
            current_range: 0.0,
            last_frame: None,
            info: ShadowTypeInfo {
                allow_updates: false,
                allow_world_align: true,
                shadow_type: decal.shadow_type,
                shadow_name: decal.texture_name.clone(),
                size_x: DECAL_WIDTH,
                size_y: DECAL_WIDTH,
                offset_x: 0.0,
                offset_y: 0.0,
            },
            color,
            marks: std::array::from_fn(|_| None),
        });
    }

    pub(crate) fn update_spy_grid_decals(&mut self, effects: &mut ScriptPresentationEffects<'_>) {
        let objects = &self.objects;
        let frame = self.frame;
        let mut allocator = effects
            .view
            .as_deref_mut()
            .and_then(|view| view.projected_radius_allocator());
        self.spy_satellites.grid_decals.0.retain_mut(|grid| {
            let Some(object) = objects.get(&grid.ping) else {
                return false;
            };
            if !object.is_alive()
                || !std::ptr::eq(
                    grid.identity.as_ptr(),
                    Arc::as_ptr(&object.visual_draw_identity_token),
                )
            {
                return false;
            }
            let elapsed = frame.saturating_sub(grid.start_frame);
            if elapsed >= grid.grow_delay.saturating_add(grid.grow_time) {
                return false;
            }
            if grid.last_frame == Some(frame) {
                return true;
            }

            if grid.last_frame.is_none() {
                if let Some(allocator) = allocator.as_deref_mut() {
                    for mark in &mut grid.marks {
                        *mark = allocator.allocate_authored_radius(&grid.info);
                        if let Some(mark) = mark {
                            mark.set_color(grid.color);
                        }
                    }
                }
            }

            let center = object.get_position();
            let radius = grid.current_range + elapsed as f32 * 2.0;
            let opacity = game_engine::base_type::real_to_int(
                255.0 * (1.0 - grid.current_range / grid.native_range),
            );
            let angle_increment = (std::f32::consts::PI * 2.0) / MARK_COUNT as f32;
            let mut angle: f32 = 0.0;
            for mark in &grid.marks {
                if let Some(mark) = mark {
                    let mut x = center.x + angle.sin() * radius;
                    let mut y = center.z + angle.cos() * radius;
                    x -= (x as i32 % GRID_SPACING) as f32;
                    y -= (y as i32 % GRID_SPACING) as f32;
                    mark.set_position(x, y, 0.0);
                    mark.set_opacity(opacity);
                }
                angle += angle_increment;
            }
            if elapsed >= grid.grow_delay {
                grid.current_range += grid.native_range / grid.grow_time.max(1) as f32;
            }
            grid.last_frame = Some(frame);
            true
        });
    }
}
