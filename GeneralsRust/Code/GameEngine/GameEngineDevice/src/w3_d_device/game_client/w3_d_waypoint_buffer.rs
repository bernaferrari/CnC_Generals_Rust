//! W3D waypoint overlay compatibility state.
//!
//! C++ source: `GameEngineDevice/Source/W3DDevice/GameClient/W3dWaypointBuffer.cpp`.
//! The original object renders waypoint/rally overlays with a node render object
//! and one segmented line object.  This Rust port preserves the render decisions
//! and path construction as CPU data so higher layers can submit equivalent
//! nodes and segmented lines without depending on C++ globals.

/// Maximum waypoint nodes displayed by C++.
pub const MAX_DISPLAY_NODES: usize = 512;

/// Maximum line points in waypoint-path mode.
pub const MAX_LINE_POINTS: usize = MAX_DISPLAY_NODES + 1;

/// C++ waypoint node render-object name.
pub const WAYPOINT_NODE_RENDER_OBJECT: &str = "SCMNode";

/// C++ waypoint line texture name.
pub const WAYPOINT_LINE_TEXTURE: &str = "EXLaser.tga";

/// C++ `Vector3` subset.
pub type Vector3 = [f32; 3];

/// Segmented-line texture mapping mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum WaypointTextureMapMode {
    /// Stretch texture mapping.
    Stretch = 0,
    /// C++ `TILED_TEXTURE_MAP`.
    Tiled = 1,
}

/// Depth compare mode used by the waypoint line shader.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum WaypointDepthCompare {
    /// C++ `ShaderClass::PASS_ALWAYS`.
    PassAlways = 0,
}

/// CPU representation of C++ segmented-line style.
#[derive(Debug, Clone, PartialEq)]
pub struct WaypointLineStyle {
    /// Optional texture name.
    pub texture_name: Option<String>,
    /// Whether C++ uses the additive shader preset.
    pub additive_shader: bool,
    /// Depth compare mode.
    pub depth_compare: WaypointDepthCompare,
    /// Line width.
    pub width: f32,
    /// RGB color.
    pub color: Vector3,
    /// Texture mapping mode.
    pub texture_map_mode: WaypointTextureMapMode,
}

impl WaypointLineStyle {
    fn default_with_texture(texture_name: Option<String>) -> Self {
        Self {
            texture_name,
            additive_shader: true,
            depth_compare: WaypointDepthCompare::PassAlways,
            width: 1.5,
            color: [0.25, 0.5, 1.0],
            texture_map_mode: WaypointTextureMapMode::Tiled,
        }
    }
}

/// One segmented-line render decision.
#[derive(Debug, Clone, PartialEq)]
pub struct WaypointLineDraw {
    /// Points passed to `SegmentedLineClass::Set_Points`.
    pub points: Vec<Vector3>,
    /// Style active for this line.
    pub style: WaypointLineStyle,
}

/// Render decisions emitted by one `drawWaypoints` call.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WaypointDrawFrame {
    /// Positions where C++ renders `SCMNode`.
    pub node_positions: Vec<Vector3>,
    /// Segmented lines to render.
    pub lines: Vec<WaypointLineDraw>,
}

/// Relationship between the selected revealer and the moused-over object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum WaypointRelationship {
    /// C++ `ENEMIES`.
    Enemies = 0,
    /// Any non-enemy relationship.
    Other = 1,
}

/// AI path data consumed by the C++ waypoint friend accessors.
#[derive(Debug, Clone, PartialEq)]
pub struct WaypointAiPath {
    /// `friend_getWaypointGoalPathSize`.
    pub goal_path: Vec<Option<Vector3>>,
    /// `friend_getCurrentGoalPathIndex`.
    pub current_goal_path_index: i32,
    /// `getGoalPosition`, used by the enemy-path reveal fallback.
    pub goal_position: Option<Vector3>,
}

impl WaypointAiPath {
    /// Construct a waypoint path.
    #[must_use]
    pub fn new(goal_path: Vec<Option<Vector3>>, current_goal_path_index: i32) -> Self {
        Self {
            goal_path,
            current_goal_path_index,
            goal_position: None,
        }
    }
}

/// Geometry fields used by the rally box-wrap path.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaypointGeometry {
    /// C++ major radius.
    pub major_radius: f32,
    /// C++ minor radius.
    pub minor_radius: f32,
}

/// Exit/rally interface data used by the non-waypoint branch.
#[derive(Debug, Clone, PartialEq)]
pub struct WaypointExitInterface {
    /// `getExitPosition`, falling back to the object position when absent.
    pub exit_position: Option<Vector3>,
    /// `getNaturalRallyPoint(..., FALSE)`.
    pub natural_rally_point: Option<Vector3>,
    /// `getRallyPoint`.
    pub rally_point: Option<Vector3>,
}

/// Moused-over object data for `KINDOF_REVEALS_ENEMY_PATHS`.
#[derive(Debug, Clone, PartialEq)]
pub struct MousedOverWaypointObject {
    /// Moused-over object.
    pub object: WaypointObject,
    /// Relationship to the selected revealer.
    pub relationship_to_selected: WaypointRelationship,
}

/// Drawable/object state needed by `drawWaypoints`.
#[derive(Debug, Clone, PartialEq)]
pub struct WaypointObject {
    /// Stable id, useful to callers and tests.
    pub id: u32,
    /// Object position.
    pub position: Vector3,
    /// Object orientation in radians.
    pub orientation: f32,
    /// `KINDOF_IGNORED_IN_GUI`.
    pub ignored_in_gui: bool,
    /// `isLocallyControlled`.
    pub locally_controlled: bool,
    /// `KINDOF_REVEALS_ENEMY_PATHS`.
    pub reveals_enemy_paths: bool,
    /// Vision range for enemy path reveal.
    pub vision_range: f32,
    /// Optional AI path.
    pub ai: Option<WaypointAiPath>,
    /// Optional exit/rally interface.
    pub exit_interface: Option<WaypointExitInterface>,
    /// Object geometry for rally box-wrap.
    pub geometry: WaypointGeometry,
}

impl WaypointObject {
    /// Construct an object with C++-neutral defaults.
    #[must_use]
    pub fn new(id: u32, position: Vector3) -> Self {
        Self {
            id,
            position,
            orientation: 0.0,
            ignored_in_gui: false,
            locally_controlled: true,
            reveals_enemy_paths: false,
            vision_range: 0.0,
            ai: None,
            exit_interface: None,
            geometry: WaypointGeometry {
                major_radius: 0.0,
                minor_radius: 0.0,
            },
        }
    }
}

/// Input snapshot replacing C++ globals used by `drawWaypoints`.
#[derive(Debug, Clone, PartialEq)]
pub struct WaypointDrawContext {
    /// `TheInGameUI->isInWaypointMode`.
    pub in_waypoint_mode: bool,
    /// `TheInGameUI->getAllSelectedDrawables`.
    pub selected: Vec<WaypointObject>,
    /// Moused-over drawable object, if any.
    pub moused_over: Option<MousedOverWaypointObject>,
}

/// W3D waypoint overlay buffer.
#[derive(Debug, Clone)]
pub struct W3DWaypointBuffer {
    waypoint_node_render_object: Option<String>,
    line_texture_name: Option<String>,
    line_style: WaypointLineStyle,
}

impl W3DWaypointBuffer {
    /// Construct with the C++ asset names available.
    #[must_use]
    pub fn new() -> Self {
        let mut buffer = Self {
            waypoint_node_render_object: Some(WAYPOINT_NODE_RENDER_OBJECT.to_string()),
            line_texture_name: Some(WAYPOINT_LINE_TEXTURE.to_string()),
            line_style: WaypointLineStyle::default_with_texture(Some(
                WAYPOINT_LINE_TEXTURE.to_string(),
            )),
        };
        buffer.set_default_line_style();
        buffer
    }

    /// Construct with explicit asset availability.
    #[must_use]
    pub fn with_assets(has_waypoint_node: bool, has_texture: bool) -> Self {
        let mut buffer = Self {
            waypoint_node_render_object: has_waypoint_node
                .then(|| WAYPOINT_NODE_RENDER_OBJECT.to_string()),
            line_texture_name: has_texture.then(|| WAYPOINT_LINE_TEXTURE.to_string()),
            line_style: WaypointLineStyle::default_with_texture(None),
        };
        buffer.set_default_line_style();
        buffer
    }

    /// C++ `freeWaypointBuffers`, intentionally empty.
    pub const fn free_waypoint_buffers(&self) {}

    /// C++ `setDefaultLineStyle`.
    pub fn set_default_line_style(&mut self) {
        self.line_style = WaypointLineStyle::default_with_texture(self.line_texture_name.clone());
    }

    /// C++ `drawWaypoints`.
    pub fn draw_waypoints(&mut self, context: Option<&WaypointDrawContext>) -> WaypointDrawFrame {
        let Some(context) = context else {
            return WaypointDrawFrame::default();
        };

        self.set_default_line_style();
        if context.in_waypoint_mode {
            self.draw_waypoint_mode(context)
        } else {
            self.draw_rally_or_revealed_paths(context)
        }
    }

    /// Current default line style.
    #[must_use]
    pub fn line_style(&self) -> &WaypointLineStyle {
        &self.line_style
    }

    /// Waypoint node render-object name.
    #[must_use]
    pub fn waypoint_node_render_object(&self) -> Option<&str> {
        self.waypoint_node_render_object.as_deref()
    }

    /// Waypoint line texture name.
    #[must_use]
    pub fn line_texture_name(&self) -> Option<&str> {
        self.line_texture_name.as_deref()
    }

    fn draw_waypoint_mode(&self, context: &WaypointDrawContext) -> WaypointDrawFrame {
        let mut frame = WaypointDrawFrame::default();

        for object in &context.selected {
            if object.ignored_in_gui {
                continue;
            }

            let Some(ai) = object.ai.as_ref() else {
                continue;
            };
            let goal_size = ai.goal_path.len() as i32;
            let gp_idx = ai.current_goal_path_index;
            if gp_idx < 0 || gp_idx >= goal_size {
                continue;
            }

            let mut points = vec![object.position];
            for waypoint in ai.goal_path.iter().skip(gp_idx as usize).flatten() {
                if points.len() < MAX_LINE_POINTS {
                    points.push(*waypoint);
                }
                self.push_node(&mut frame, *waypoint);
            }

            frame.lines.push(WaypointLineDraw {
                points,
                style: self.line_style.clone(),
            });
        }

        frame
    }

    fn draw_rally_or_revealed_paths(&mut self, context: &WaypointDrawContext) -> WaypointDrawFrame {
        let mut frame = WaypointDrawFrame::default();

        for object in &context.selected {
            if !object.locally_controlled {
                continue;
            }

            if object.reveals_enemy_paths {
                self.draw_revealed_enemy_path(context, object, &mut frame);
                break;
            }

            self.draw_rally_path(object, &mut frame);
        }

        frame
    }

    fn draw_revealed_enemy_path(
        &mut self,
        context: &WaypointDrawContext,
        revealer: &WaypointObject,
        frame: &mut WaypointDrawFrame,
    ) {
        let Some(moused_over) = context.moused_over.as_ref() else {
            return;
        };
        if moused_over.relationship_to_selected != WaypointRelationship::Enemies {
            return;
        }

        let enemy = &moused_over.object;
        if distance(revealer.position, enemy.position) > revealer.vision_range {
            return;
        }

        let Some(ai) = enemy.ai.as_ref() else {
            return;
        };

        let mut points = vec![enemy.position];
        let mut line_exists = false;
        let goal_size = ai.goal_path.len() as i32;
        let gp_idx = ai.current_goal_path_index;

        if gp_idx >= 0 && gp_idx < goal_size {
            for waypoint in ai.goal_path.iter().skip(gp_idx as usize).flatten() {
                if points.len() < MAX_LINE_POINTS {
                    points.push(*waypoint);
                }
                self.push_node(frame, *waypoint);
                line_exists = true;
            }
        } else if let Some(destination) = ai.goal_position {
            if length(destination) > 1.0 {
                points.push(destination);
                self.push_node(frame, destination);
                line_exists = true;
            }
        }

        if line_exists {
            let mut style = self.line_style.clone();
            style.color = [0.95, 0.5, 0.0];
            style.width = 3.0;
            frame.lines.push(WaypointLineDraw { points, style });
        }
    }

    fn draw_rally_path(&self, object: &WaypointObject, frame: &mut WaypointDrawFrame) {
        let Some(exit_interface) = object.exit_interface.as_ref() else {
            return;
        };

        let exit_point = exit_interface.exit_position.unwrap_or(object.position);
        let mut points = vec![exit_point];
        let mut box_wrap = true;

        let Some(natural_rally_point) = exit_interface.natural_rally_point else {
            return;
        };
        if equals(natural_rally_point, exit_point) {
            box_wrap = false;
        } else {
            points.push(natural_rally_point);
        }

        let Some(rally_point) = exit_interface.rally_point else {
            return;
        };

        if box_wrap {
            self.append_box_wrap_points(
                object,
                exit_point,
                natural_rally_point,
                rally_point,
                &mut points,
                frame,
            );
        }

        points.push(rally_point);
        self.push_node(frame, natural_rally_point);
        frame.lines.push(WaypointLineDraw {
            points,
            style: self.line_style.clone(),
        });
    }

    fn append_box_wrap_points(
        &self,
        object: &WaypointObject,
        exit_point: Vector3,
        natural_rally_point: Vector3,
        rally_point: Vector3,
        points: &mut Vec<Vector3>,
        frame: &mut WaypointDrawFrame,
    ) {
        let nrp_delta = [
            natural_rally_point[0] - exit_point[0],
            natural_rally_point[1] - exit_point[1],
            0.0,
        ];
        let mut way_out_point = normalize(nrp_delta);
        way_out_point = scale(way_out_point, 99_999.9);
        let way_out_length = length(way_out_point);
        way_out_point = add(way_out_point, natural_rally_point);

        let rally_to_way_out_delta = sub(way_out_point, rally_point);
        if 100.0 + length(rally_to_way_out_delta) <= way_out_length {
            return;
        }

        let way_out_normal = normalize(way_out_point);
        let nrp_to_rp_delta = normalize(sub(natural_rally_point, rally_point));
        let mut dot_value = dot2(nrp_to_rp_delta, way_out_normal);
        if dot_value <= 0.0 {
            return;
        }

        let angle = object.orientation;
        let c = angle.cos();
        let s = angle.sin();
        let exc = object.geometry.major_radius * c;
        let eyc = object.geometry.minor_radius * c;
        let exs = object.geometry.major_radius * s;
        let eys = object.geometry.minor_radius * s;
        let center = object.position;
        let corners = [
            [center[0] - exc - eys, center[1] + eyc - exs],
            [center[0] + exc - eys, center[1] + eyc + exs],
            [center[0] + exc + eys, center[1] - eyc + exs],
            [center[0] - exc + eys, center[1] - eyc - exs],
        ];

        let mut near_elbow: Option<[f32; 2]> = None;
        let mut far_elbow: Option<[f32; 2]> = None;
        let mut elbow_distance_near = 99_999.9;
        let mut elbow_distance_far = 99_999.9;

        for corner in corners {
            let corner_to_exit_delta =
                normalize([exit_point[0] - corner[0], exit_point[1] - corner[1], 0.0]);
            dot_value = dot2(corner_to_exit_delta, way_out_normal);
            let corner_to_rp_delta = [rally_point[0] - corner[0], rally_point[1] - corner[1], 0.0];
            let corner_to_rp_len = length(corner_to_rp_delta);
            if dot_value < 0.0 {
                if corner_to_rp_len < elbow_distance_near {
                    elbow_distance_near = corner_to_rp_len;
                    near_elbow = Some(corner);
                }
            } else if corner_to_rp_len < elbow_distance_far {
                elbow_distance_far = corner_to_rp_len;
                far_elbow = Some(corner);
            }
        }

        let Some(near_elbow) = near_elbow else {
            return;
        };
        let near_point = [near_elbow[0], near_elbow[1], center[2]];
        self.push_node(frame, near_point);
        points.push(near_point);

        let Some(far_elbow) = far_elbow else {
            return;
        };
        let first_elbow_delta = normalize([
            natural_rally_point[0] - near_elbow[0],
            natural_rally_point[1] - near_elbow[1],
            0.0,
        ]);
        let first_to_rp_delta = normalize([
            near_elbow[0] - rally_point[0],
            near_elbow[1] - rally_point[1],
            0.0,
        ]);
        dot_value = dot2(first_to_rp_delta, first_elbow_delta);
        if dot_value < 0.0 {
            let far_point = [far_elbow[0], far_elbow[1], center[2]];
            self.push_node(frame, far_point);
            points.push(far_point);
        }
    }

    fn push_node(&self, frame: &mut WaypointDrawFrame, position: Vector3) {
        if self.waypoint_node_render_object.is_some() {
            frame.node_positions.push(position);
        }
    }
}

impl Default for W3DWaypointBuffer {
    fn default() -> Self {
        Self::new()
    }
}

fn equals(a: Vector3, b: Vector3) -> bool {
    a == b
}

fn add(a: Vector3, b: Vector3) -> Vector3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: Vector3, b: Vector3) -> Vector3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn scale(v: Vector3, amount: f32) -> Vector3 {
    [v[0] * amount, v[1] * amount, v[2] * amount]
}

fn length(v: Vector3) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

fn distance(a: Vector3, b: Vector3) -> f32 {
    length(sub(a, b))
}

fn normalize(v: Vector3) -> Vector3 {
    let len = length(v);
    if len > 0.0 {
        [v[0] / len, v[1] / len, v[2] / len]
    } else {
        v
    }
}

fn dot2(a: Vector3, b: Vector3) -> f32 {
    a[0] * b[0] + a[1] * b[1]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obj(id: u32, position: Vector3) -> WaypointObject {
        WaypointObject::new(id, position)
    }

    fn context(in_waypoint_mode: bool, selected: Vec<WaypointObject>) -> WaypointDrawContext {
        WaypointDrawContext {
            in_waypoint_mode,
            selected,
            moused_over: None,
        }
    }

    #[test]
    fn constructor_matches_cpp_assets_and_default_style() {
        let buffer = W3DWaypointBuffer::new();

        assert_eq!(
            buffer.waypoint_node_render_object(),
            Some(WAYPOINT_NODE_RENDER_OBJECT)
        );
        assert_eq!(buffer.line_texture_name(), Some(WAYPOINT_LINE_TEXTURE));
        assert_eq!(
            buffer.line_style(),
            &WaypointLineStyle {
                texture_name: Some(WAYPOINT_LINE_TEXTURE.to_string()),
                additive_shader: true,
                depth_compare: WaypointDepthCompare::PassAlways,
                width: 1.5,
                color: [0.25, 0.5, 1.0],
                texture_map_mode: WaypointTextureMapMode::Tiled,
            }
        );
    }

    #[test]
    fn draw_waypoints_returns_empty_without_ingame_ui() {
        let mut buffer = W3DWaypointBuffer::new();

        assert_eq!(buffer.draw_waypoints(None), WaypointDrawFrame::default());
    }

    #[test]
    fn waypoint_mode_draws_selected_ai_paths_and_skips_ignored_objects() {
        let mut selected = obj(1, [0.0, 0.0, 0.0]);
        selected.ai = Some(WaypointAiPath::new(
            vec![Some([1.0, 0.0, 0.0]), None, Some([2.0, 0.0, 0.0])],
            0,
        ));
        let mut ignored = obj(2, [10.0, 0.0, 0.0]);
        ignored.ignored_in_gui = true;
        ignored.ai = Some(WaypointAiPath::new(vec![Some([11.0, 0.0, 0.0])], 0));
        let mut buffer = W3DWaypointBuffer::new();

        let frame = buffer.draw_waypoints(Some(&context(true, vec![selected, ignored])));

        assert_eq!(frame.node_positions, vec![[1.0, 0.0, 0.0], [2.0, 0.0, 0.0]]);
        assert_eq!(frame.lines.len(), 1);
        assert_eq!(
            frame.lines[0].points,
            vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]]
        );
    }

    #[test]
    fn waypoint_mode_caps_line_points_but_renders_all_nodes() {
        let mut selected = obj(1, [0.0, 0.0, 0.0]);
        selected.ai = Some(WaypointAiPath::new(
            (0..MAX_LINE_POINTS + 5)
                .map(|index| Some([index as f32, 1.0, 0.0]))
                .collect(),
            0,
        ));
        let mut buffer = W3DWaypointBuffer::new();

        let frame = buffer.draw_waypoints(Some(&context(true, vec![selected])));

        assert_eq!(frame.lines[0].points.len(), MAX_LINE_POINTS);
        assert_eq!(frame.node_positions.len(), MAX_LINE_POINTS + 5);
    }

    #[test]
    fn non_waypoint_reveals_one_enemy_path_with_orange_style_then_breaks() {
        let mut revealer = obj(1, [0.0, 0.0, 0.0]);
        revealer.reveals_enemy_paths = true;
        revealer.vision_range = 100.0;
        let mut second = obj(2, [0.0, 0.0, 0.0]);
        second.exit_interface = Some(WaypointExitInterface {
            exit_position: Some([0.0, 0.0, 0.0]),
            natural_rally_point: Some([1.0, 0.0, 0.0]),
            rally_point: Some([2.0, 0.0, 0.0]),
        });
        let mut enemy = obj(99, [5.0, 0.0, 0.0]);
        enemy.ai = Some(WaypointAiPath::new(vec![Some([6.0, 0.0, 0.0])], 0));
        let draw_context = WaypointDrawContext {
            in_waypoint_mode: false,
            selected: vec![revealer, second],
            moused_over: Some(MousedOverWaypointObject {
                object: enemy,
                relationship_to_selected: WaypointRelationship::Enemies,
            }),
        };
        let mut buffer = W3DWaypointBuffer::new();

        let frame = buffer.draw_waypoints(Some(&draw_context));

        assert_eq!(frame.node_positions, vec![[6.0, 0.0, 0.0]]);
        assert_eq!(frame.lines.len(), 1);
        assert_eq!(
            frame.lines[0].points,
            vec![[5.0, 0.0, 0.0], [6.0, 0.0, 0.0]]
        );
        assert_eq!(frame.lines[0].style.color, [0.95, 0.5, 0.0]);
        assert_eq!(frame.lines[0].style.width, 3.0);
    }

    #[test]
    fn revealed_enemy_path_uses_goal_position_when_no_current_path() {
        let mut revealer = obj(1, [0.0, 0.0, 0.0]);
        revealer.reveals_enemy_paths = true;
        revealer.vision_range = 100.0;
        let mut enemy = obj(99, [5.0, 0.0, 0.0]);
        let mut ai = WaypointAiPath::new(Vec::new(), -1);
        ai.goal_position = Some([10.0, 0.0, 0.0]);
        enemy.ai = Some(ai);
        let draw_context = WaypointDrawContext {
            in_waypoint_mode: false,
            selected: vec![revealer],
            moused_over: Some(MousedOverWaypointObject {
                object: enemy,
                relationship_to_selected: WaypointRelationship::Enemies,
            }),
        };
        let mut buffer = W3DWaypointBuffer::new();

        let frame = buffer.draw_waypoints(Some(&draw_context));

        assert_eq!(frame.node_positions, vec![[10.0, 0.0, 0.0]]);
        assert_eq!(
            frame.lines[0].points,
            vec![[5.0, 0.0, 0.0], [10.0, 0.0, 0.0]]
        );
    }

    #[test]
    fn non_waypoint_draws_simple_rally_path_and_natural_node() {
        let mut building = obj(1, [0.0, 0.0, 0.0]);
        building.exit_interface = Some(WaypointExitInterface {
            exit_position: Some([0.0, 0.0, 0.0]),
            natural_rally_point: Some([1.0, 0.0, 0.0]),
            rally_point: Some([2.0, 0.0, 0.0]),
        });
        let mut buffer = W3DWaypointBuffer::new();

        let frame = buffer.draw_waypoints(Some(&context(false, vec![building])));

        assert_eq!(frame.node_positions, vec![[1.0, 0.0, 0.0]]);
        assert_eq!(
            frame.lines[0].points,
            vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]]
        );
    }

    #[test]
    fn rally_path_helipad_case_disables_box_wrap() {
        let mut building = obj(1, [0.0, 0.0, 0.0]);
        building.geometry = WaypointGeometry {
            major_radius: 10.0,
            minor_radius: 10.0,
        };
        building.exit_interface = Some(WaypointExitInterface {
            exit_position: Some([0.0, 0.0, 0.0]),
            natural_rally_point: Some([0.0, 0.0, 0.0]),
            rally_point: Some([-20.0, 0.0, 0.0]),
        });
        let mut buffer = W3DWaypointBuffer::new();

        let frame = buffer.draw_waypoints(Some(&context(false, vec![building])));

        assert_eq!(frame.node_positions, vec![[0.0, 0.0, 0.0]]);
        assert_eq!(
            frame.lines[0].points,
            vec![[0.0, 0.0, 0.0], [-20.0, 0.0, 0.0]]
        );
    }

    #[test]
    fn missing_node_asset_suppresses_node_draws_but_keeps_lines() {
        let mut selected = obj(1, [0.0, 0.0, 0.0]);
        selected.ai = Some(WaypointAiPath::new(vec![Some([1.0, 0.0, 0.0])], 0));
        let mut buffer = W3DWaypointBuffer::with_assets(false, true);

        let frame = buffer.draw_waypoints(Some(&context(true, vec![selected])));

        assert!(frame.node_positions.is_empty());
        assert_eq!(frame.lines.len(), 1);
    }
}
