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
pub struct InGameUIDrawList {
    pub selection_rectangles: Vec<(ScreenRect, u32)>,
    pub placement_lines: Vec<WorldLine>,
    pub rally_indicators: Vec<RingIndicator>,
    pub superweapon_reticles: Vec<RingIndicator>,
}

impl Default for InGameUIDrawList {
    fn default() -> Self {
        Self {
            selection_rectangles: Vec::new(),
            placement_lines: Vec::new(),
            rally_indicators: Vec::new(),
            superweapon_reticles: Vec::new(),
        }
    }
}

#[derive(Debug, Default)]
pub struct W3DInGameUI {
    selection_region: Option<(Vec2, Vec2)>,
    placement_anchor: Option<Vec3>,
    placement_facing: Option<Vec3>,
    rally_points: Vec<Vec3>,
    superweapon_target: Option<Vec3>,
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
    }

    pub fn set_rally_points(&mut self, rally_points: Vec<Vec3>) {
        self.rally_points = rally_points;
    }

    pub fn set_superweapon_target(&mut self, target: Option<Vec3>) {
        self.superweapon_target = target;
    }

    pub fn draw(&self) -> InGameUIDrawList {
        let mut draw_list = InGameUIDrawList::default();
        self.draw_selection_region(&mut draw_list);
        self.draw_place_angle(&mut draw_list);
        self.draw_rally_points(&mut draw_list);
        self.draw_superweapon_targeting(&mut draw_list);
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

    fn draw_place_angle(&self, draw_list: &mut InGameUIDrawList) {
        if let (Some(anchor), Some(facing)) = (self.placement_anchor, self.placement_facing) {
            let arrow_end = anchor + facing * 20.0;
            draw_list.placement_lines.push(WorldLine {
                start: anchor,
                end: arrow_end,
                color: 0xffff_ff00,
            });
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
        ui.set_rally_points(vec![Vec3::new(1.0, 2.0, 3.0)]);
        ui.set_superweapon_target(Some(Vec3::new(5.0, 6.0, 7.0)));
        let draw = ui.draw();
        assert_eq!(draw.placement_lines.len(), 1);
        assert_eq!(draw.rally_indicators.len(), 1);
        assert_eq!(draw.superweapon_reticles.len(), 2);
    }
}
