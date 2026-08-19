use glam::{Vec2, Vec3};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenRect {
    pub min: Vec2,
    pub max: Vec2,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldLine {
    pub start: Vec3,
    pub end: Vec3,
    pub color: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RingIndicator {
    pub center: Vec3,
    pub radius: f32,
    pub color: u32,
    pub segments: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SceneModel {
    pub name: String,
    pub position: Vec3,
    pub hidden: bool,
    pub anim_mode_once: bool,
    pub in_scene: bool,
}

impl SceneModel {
    fn new(name: &str, position: Vec3, anim_mode_once: bool) -> Self {
        Self {
            name: name.to_string(),
            position,
            hidden: false,
            anim_mode_once,
            in_scene: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct InGameUIDrawList {
    pub selection_rectangles: Vec<(ScreenRect, u32)>,
    pub placement_lines: Vec<WorldLine>,
    pub rally_indicators: Vec<RingIndicator>,
    pub superweapon_reticles: Vec<RingIndicator>,
    pub scene_models: Vec<SceneModel>,
}

impl Default for InGameUIDrawList {
    fn default() -> Self {
        Self {
            selection_rectangles: Vec::new(),
            placement_lines: Vec::new(),
            rally_indicators: Vec::new(),
            superweapon_reticles: Vec::new(),
            scene_models: Vec::new(),
        }
    }
}

#[derive(Debug, Default)]
pub struct W3DInGameUI {
    selection_region: Option<(Vec2, Vec2)>,
    placement_anchor: Option<Vec3>,
    placement_facing: Option<Vec3>,
    placement_drag_pixels: f32,
    placement_anchored: bool,
    rally_points: Vec<Vec3>,
    superweapon_target: Option<Vec3>,
    move_hint_models: Vec<Option<SceneModel>>,
    locater_anchor: Option<SceneModel>,
    locater_arrow: Option<SceneModel>,
}

impl W3DInGameUI {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn init(&mut self) {}

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn set_selection_region(&mut self, start: Vec2, end: Vec2) {
        self.selection_region = Some((start, end));
    }

    pub fn clear_selection_region(&mut self) {
        self.selection_region = None;
    }

    pub fn set_placement_cursor(&mut self, anchor: Vec3, facing: Vec3) {
        self.placement_anchor = Some(anchor);
        self.placement_facing = Some(facing.normalize_or_zero());
        self.placement_anchored = true;
    }

    pub fn set_placement_drag_pixels(&mut self, pixels: f32) {
        self.placement_drag_pixels = pixels;
    }

    pub fn set_placement_anchored(&mut self, anchored: bool) {
        self.placement_anchored = anchored;
        if !anchored {
            if let Some(model) = &mut self.locater_anchor {
                model.in_scene = false;
                model.hidden = true;
            }
            if let Some(model) = &mut self.locater_arrow {
                model.in_scene = false;
                model.hidden = true;
            }
        }
    }

    pub fn is_placement_anchored(&self) -> bool {
        self.placement_anchored
    }

    /// C++ `W3DInGameUI::drawMoveHints` — create `m_moveHintName` models for 40 frames.
    pub fn draw_move_hints(&mut self, frame: u32, hints: &[(Vec3, u32)], model_name: &str) {
        const MAX_MOVE_HINTS: usize = 256;
        if self.move_hint_models.len() < MAX_MOVE_HINTS {
            self.move_hint_models.resize(MAX_MOVE_HINTS, None);
        }
        let name = if model_name.is_empty() {
            "MoveHint"
        } else {
            model_name
        };
        for (index, model) in self.move_hint_models.iter_mut().enumerate() {
            let live = hints.get(index).and_then(|(pos, created)| {
                (frame.saturating_sub(*created) <= 40).then_some(*pos)
            });
            match (model.as_mut(), live) {
                (Some(existing), Some(pos)) => {
                    existing.position = pos;
                    existing.hidden = false;
                    existing.in_scene = true;
                    existing.anim_mode_once = true;
                }
                (None, Some(pos)) => {
                    *model = Some(SceneModel::new(name, pos, true));
                }
                (Some(existing), None) => {
                    existing.hidden = true;
                    existing.in_scene = false;
                }
                (None, None) => {}
            }
        }
        for (index, (pos, created)) in hints.iter().enumerate() {
            if index >= MAX_MOVE_HINTS {
                break;
            }
            if frame.saturating_sub(*created) > 40 {
                continue;
            }
            if self.move_hint_models[index].is_none() {
                self.move_hint_models[index] = Some(SceneModel::new(name, *pos, true));
            }
        }
    }

    pub fn draw(&mut self) -> InGameUIDrawList {
        let mut draw_list = InGameUIDrawList::default();
        self.draw_selection_region(&mut draw_list);
        self.draw_place_angle(&mut draw_list);
        self.draw_rally_points(&mut draw_list);
        self.draw_superweapon_targeting(&mut draw_list);
        for model in self.move_hint_models.iter().flatten() {
            if model.in_scene && !model.hidden {
                draw_list.scene_models.push(model.clone());
            }
        }
        if let Some(model) = &self.locater_anchor {
            if model.in_scene && !model.hidden {
                draw_list.scene_models.push(model.clone());
            }
        }
        if let Some(model) = &self.locater_arrow {
            if model.in_scene && !model.hidden {
                draw_list.scene_models.push(model.clone());
            }
        }
        draw_list
    }

    fn draw_selection_region(&self, draw_list: &mut InGameUIDrawList) {
        if let Some((start, end)) = self.selection_region {
            draw_list.selection_rectangles.push((
                ScreenRect {
                    min: start.min(end),
                    max: start.max(end),
                },
                0x40_00ff_00,
            ));
        }
    }

    fn draw_place_angle(&mut self, draw_list: &mut InGameUIDrawList) {
        if self.locater_anchor.is_none() {
            self.locater_anchor = Some(SceneModel::new("Locater01", Vec3::ZERO, false));
            if let Some(model) = &mut self.locater_anchor {
                model.in_scene = false;
                model.hidden = true;
            }
        }
        if self.locater_arrow.is_none() {
            self.locater_arrow = Some(SceneModel::new("Locater02", Vec3::ZERO, false));
            if let Some(model) = &mut self.locater_arrow {
                model.in_scene = false;
                model.hidden = true;
            }
        }

        if !self.placement_anchored {
            if let Some(model) = &mut self.locater_anchor {
                model.in_scene = false;
                model.hidden = true;
            }
            if let Some(model) = &mut self.locater_arrow {
                model.in_scene = false;
                model.hidden = true;
            }
            return;
        }

        if let (Some(anchor), Some(facing)) = (self.placement_anchor, self.placement_facing) {
            let show_arrow = self.placement_drag_pixels >= 5.0;
            if let Some(model) = &mut self.locater_anchor {
                model.position = anchor;
                model.hidden = show_arrow;
                model.in_scene = !show_arrow;
            }
            if let Some(model) = &mut self.locater_arrow {
                model.position = anchor;
                model.hidden = !show_arrow;
                model.in_scene = show_arrow;
            }
            if show_arrow {
                draw_list.placement_lines.push(WorldLine {
                    start: anchor,
                    end: anchor + facing * 20.0,
                    color: 0xffff_ff00,
                });
            }
        }
    }

    fn draw_rally_points(&self, draw_list: &mut InGameUIDrawList) {
        for point in &self.rally_points {
            draw_list.rally_indicators.push(RingIndicator {
                center: *point,
                radius: 6.0,
                color: 0xff00_ffff,
                segments: 24,
            });
        }
    }

    fn draw_superweapon_targeting(&self, draw_list: &mut InGameUIDrawList) {
        if let Some(target) = self.superweapon_target {
            draw_list.superweapon_reticles.push(RingIndicator {
                center: target,
                radius: 18.0,
                color: 0xffff_0000,
                segments: 32,
            });
            draw_list.superweapon_reticles.push(RingIndicator {
                center: target,
                radius: 32.0,
                color: 0x80ff_8000,
                segments: 32,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_selection_rectangle() {
        let mut ui = W3DInGameUI::new();
        ui.set_selection_region(Vec2::new(50.0, 100.0), Vec2::new(10.0, 20.0));
        let draw = ui.draw();
        assert_eq!(draw.selection_rectangles.len(), 1);
        assert_eq!(draw.selection_rectangles[0].0.min, Vec2::new(10.0, 20.0));
    }

    #[test]
    fn emits_overlay_hints() {
        let mut ui = W3DInGameUI::new();
        ui.set_placement_cursor(Vec3::ZERO, Vec3::X);
        ui.set_placement_drag_pixels(8.0);
        ui.set_rally_points(vec![Vec3::new(1.0, 2.0, 3.0)]);
        ui.set_superweapon_target(Some(Vec3::new(5.0, 6.0, 7.0)));
        let draw = ui.draw();
        assert_eq!(draw.placement_lines.len(), 1);
        assert!(draw.scene_models.iter().any(|m| m.name == "Locater02"));
        assert_eq!(draw.rally_indicators.len(), 1);
        assert_eq!(draw.superweapon_reticles.len(), 2);
    }

    #[test]
    fn draw_move_hints_creates_models_for_40_frames() {
        let mut ui = W3DInGameUI::new();
        ui.draw_move_hints(10, &[(Vec3::new(4.0, 5.0, 1.0), 10)], "MoveHint");
        let draw = ui.draw();
        assert!(draw.scene_models.iter().any(|m| m.name == "MoveHint"));
        ui.draw_move_hints(60, &[(Vec3::new(4.0, 5.0, 1.0), 10)], "MoveHint");
        let draw = ui.draw();
        assert!(!draw.scene_models.iter().any(|m| m.name == "MoveHint"));
    }
}
