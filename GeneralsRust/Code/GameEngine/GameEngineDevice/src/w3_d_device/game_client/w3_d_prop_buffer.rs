//! W3D prop buffer compatibility state.
//!
//! C++ source: `GameEngineDevice/Source/W3DDevice/GameClient/W3DPropBuffer.cpp`.
//! The original buffer owns cloned render objects for map props, culls them,
//! refreshes shroud state, and emits render calls.  This Rust port preserves the
//! prop/type bookkeeping and draw filtering as CPU state so the renderer can
//! consume the same decisions without requiring a live W3D device in tests.

use std::collections::HashMap;

use game_engine::common::game_common::ObjectShroudStatus;
use game_engine::map_object::Coord3D;

/// Maximum props in the C++ fixed array.
pub const MAX_PROPS: usize = 4_000;

/// Maximum distinct prop render-object types.
pub const MAX_TYPES: usize = 64;

/// C++ `Vector3` subset.
pub type Vector3 = [f32; 3];

/// C++ `SphereClass` subset used for prop culling.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SphereClass {
    /// Sphere center.
    pub center: Vector3,
    /// Sphere radius.
    pub radius: f32,
}

impl SphereClass {
    /// Construct a sphere.
    pub const fn new(center: Vector3, radius: f32) -> Self {
        Self { center, radius }
    }

    fn translated(self, location: &Coord3D) -> Self {
        Self {
            center: [
                self.center[0] + location.x,
                self.center[1] + location.y,
                self.center[2] + location.z,
            ],
            radius: self.radius,
        }
    }
}

impl Default for SphereClass {
    fn default() -> Self {
        Self {
            center: [0.0; 3],
            radius: 1.0,
        }
    }
}

/// Construction geometry type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PropGeometryType {
    /// C++ `GEOMETRY_SPHERE`.
    Sphere = 0,
    /// C++ `GEOMETRY_CYLINDER`.
    Cylinder = 1,
    /// C++ `GEOMETRY_BOX`.
    Box = 2,
}

/// Geometry passed to the construction-removal collision callback.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConstructionPropGeometry {
    /// Geometry kind.
    pub geometry_type: PropGeometryType,
    /// C++ `m_isSmall`.
    pub is_small: bool,
    /// C++ geometry height.
    pub height: f32,
    /// C++ major radius.
    pub major_radius: f32,
    /// C++ minor radius.
    pub minor_radius: f32,
}

impl ConstructionPropGeometry {
    fn cylinder_for_prop_radius(radius: f32) -> Self {
        Self {
            geometry_type: PropGeometryType::Cylinder,
            is_small: false,
            height: 5.0 * radius,
            major_radius: 2.0 * radius,
            minor_radius: 2.0 * radius,
        }
    }
}

/// Cell shroud state used by the terrain shroud render pass decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CellShroudStatus {
    /// C++ `CELLSHROUD_CLEAR`.
    Clear = 0,
    /// Any non-clear terrain-cell shroud state.
    Shrouded = 1,
}

/// Cached prop type, equivalent to C++ `TPropType`.
#[derive(Debug, Clone, PartialEq)]
pub struct PropType {
    /// Model/render-object name.
    pub model_name: String,
    /// Base model bounding sphere.
    pub bounds: SphereClass,
    /// Whether the prototype render object exists.
    pub render_object_alive: bool,
}

/// Individual prop, equivalent to C++ `TProp`.
#[derive(Debug, Clone)]
pub struct Prop {
    /// Whether the cloned render object exists.
    pub render_object_alive: bool,
    /// Map object id.
    pub id: i32,
    /// Drawing location.
    pub location: Coord3D,
    /// Index into `prop_types`, or `-1` after removal.
    pub prop_type: i32,
    /// Cached shroud status.
    pub shroud_status: ObjectShroudStatus,
    /// Culling result used by draw.
    pub visible: bool,
    /// Translated bounding sphere.
    pub bounds: SphereClass,
    /// Z rotation.
    pub angle: f32,
    /// Object scale.
    pub scale: f32,
}

impl Prop {
    fn removed(id: i32) -> Self {
        Self {
            render_object_alive: false,
            id,
            location: Coord3D::default(),
            prop_type: -1,
            shroud_status: ObjectShroudStatus::Invalid,
            visible: false,
            bounds: SphereClass::default(),
            angle: 0.0,
            scale: 1.0,
        }
    }
}

/// One render decision from C++ `drawProps`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PropDrawCall {
    /// Prop id to render.
    pub id: i32,
    /// Whether C++ would push the prop shroud material pass.
    pub uses_shroud_material_pass: bool,
}

/// W3D prop draw buffer.
#[derive(Debug, Clone)]
pub struct W3DPropBuffer {
    props: Vec<Prop>,
    prop_types: Vec<PropType>,
    available_model_bounds: HashMap<String, SphereClass>,
    anything_changed: bool,
    initialized: bool,
    do_cull: bool,
}

impl W3DPropBuffer {
    /// Construct an initialized C++ prop buffer.
    pub fn new() -> Self {
        let mut buffer = Self {
            props: Vec::new(),
            prop_types: Vec::new(),
            available_model_bounds: HashMap::new(),
            anything_changed: false,
            initialized: false,
            do_cull: false,
        };
        buffer.clear_all_props();
        buffer.initialized = true;
        buffer
    }

    /// Register a model lookup result, replacing the C++ `WW3DAssetManager` query.
    pub fn register_model_bounds(&mut self, model_name: &str, bounds: SphereClass) {
        self.available_model_bounds
            .insert(canonical_model_name(model_name), bounds);
    }

    /// C++ `addPropType`.
    ///
    /// Returns `-1` when the model is unavailable.  Preserves the C++ overflow
    /// quirk of returning `0` when `MAX_TYPES` has already been reached.
    pub fn add_prop_type(&mut self, model_name: &str) -> i32 {
        if self.prop_types.len() >= MAX_TYPES {
            return 0;
        }

        let Some(bounds) = self
            .available_model_bounds
            .get(&canonical_model_name(model_name))
            .copied()
        else {
            return -1;
        };

        self.prop_types.push(PropType {
            model_name: model_name.to_string(),
            bounds,
            render_object_alive: true,
        });
        (self.prop_types.len() - 1) as i32
    }

    /// C++ `addProp`.
    pub fn add_prop(
        &mut self,
        id: i32,
        location: Coord3D,
        angle: f32,
        scale: f32,
        model_name: &str,
    ) {
        if self.props.len() >= MAX_PROPS || !self.initialized {
            return;
        }

        let mut prop_type = self.find_prop_type(model_name);
        if prop_type < 0 {
            prop_type = self.add_prop_type(model_name);
            if prop_type < 0 {
                return;
            }
        }

        let base_bounds = self.prop_types[prop_type as usize].bounds;
        self.props.push(Prop {
            render_object_alive: true,
            id,
            location: location.clone(),
            prop_type,
            shroud_status: ObjectShroudStatus::Invalid,
            visible: false,
            bounds: base_bounds.translated(&location),
            angle,
            scale,
        });
    }

    /// C++ `updatePropPosition`.
    pub fn update_prop_position(
        &mut self,
        id: i32,
        location: Coord3D,
        angle: f32,
        scale: f32,
    ) -> bool {
        for prop in &mut self.props {
            if prop.id == id && prop.render_object_alive && prop.prop_type >= 0 {
                let base_bounds = self.prop_types[prop.prop_type as usize].bounds;
                prop.location = location.clone();
                prop.angle = angle;
                prop.scale = scale;
                prop.bounds = base_bounds.translated(&location);
                self.anything_changed = true;
                return true;
            }
        }
        false
    }

    /// C++ `removeProp`.
    pub fn remove_prop(&mut self, id: i32) {
        for prop in &mut self.props {
            if prop.id == id {
                *prop = Prop::removed(id);
                self.anything_changed = true;
            }
        }
    }

    /// C++ `removePropsForConstruction`.
    pub fn remove_props_for_construction<F>(&mut self, mut collides: F)
    where
        F: FnMut(&Coord3D, ConstructionPropGeometry) -> bool,
    {
        for prop in &mut self.props {
            if !prop.render_object_alive {
                continue;
            }
            let info = ConstructionPropGeometry::cylinder_for_prop_radius(prop.bounds.radius);
            if collides(&prop.location, info) {
                let id = prop.id;
                *prop = Prop::removed(id);
                self.anything_changed = true;
            }
        }
    }

    /// C++ `notifyShroudChanged`.
    pub fn notify_shroud_changed(&mut self, has_partition_manager: bool) {
        let status = if has_partition_manager {
            ObjectShroudStatus::Invalid
        } else {
            ObjectShroudStatus::Clear
        };
        for prop in &mut self.props {
            prop.shroud_status = status;
        }
    }

    /// C++ `doFullUpdate`.
    pub fn do_full_update(&mut self) {
        self.do_cull = true;
    }

    /// C++ `drawProps`, expressed as CPU render decisions.
    pub fn draw_props<C, S>(
        &mut self,
        mut cull_sphere: C,
        mut shroud_status_for_prop: S,
        has_player_list: bool,
        has_partition_manager: bool,
        terrain_shroud: Option<CellShroudStatus>,
    ) -> Vec<PropDrawCall>
    where
        C: FnMut(&SphereClass) -> bool,
        S: FnMut(&Coord3D) -> ObjectShroudStatus,
    {
        if self.do_cull {
            self.cull(&mut cull_sphere);
        }

        let mut calls = Vec::new();
        for prop in &mut self.props {
            if !prop.visible || !prop.render_object_alive {
                continue;
            }
            if !has_player_list || !has_partition_manager {
                prop.shroud_status = ObjectShroudStatus::Clear;
            }
            if prop.shroud_status == ObjectShroudStatus::Invalid {
                prop.shroud_status = shroud_status_for_prop(&prop.location);
            }
            if shroud_rank(prop.shroud_status) >= shroud_rank(ObjectShroudStatus::Shrouded) {
                continue;
            }
            if shroud_rank(prop.shroud_status) <= shroud_rank(ObjectShroudStatus::Invalid) {
                continue;
            }

            calls.push(PropDrawCall {
                id: prop.id,
                uses_shroud_material_pass: terrain_shroud.is_some()
                    && shroud_rank(prop.shroud_status) != CellShroudStatus::Clear as u8,
            });
        }
        calls
    }

    /// C++ `crc`, intentionally empty.
    pub const fn crc(&self) {}

    /// C++ `xfer`, version one with no payload.
    pub const fn xfer_version(&self) -> u16 {
        1
    }

    /// C++ `loadPostProcess`, intentionally empty.
    pub const fn load_post_process(&self) {}

    /// Number of props, matching C++ `m_numProps`.
    pub fn num_props(&self) -> usize {
        self.props.len()
    }

    /// Number of cached prop types.
    pub fn num_prop_types(&self) -> usize {
        self.prop_types.len()
    }

    /// Current prop records.
    pub fn props(&self) -> &[Prop] {
        &self.props
    }

    /// Current prop type records.
    pub fn prop_types(&self) -> &[PropType] {
        &self.prop_types
    }

    /// Whether anything changed since construction/clear.
    pub fn anything_changed(&self) -> bool {
        self.anything_changed
    }

    /// Whether the buffer initialized successfully.
    pub fn initialized(&self) -> bool {
        self.initialized
    }

    /// C++ `clearAllProps`.
    pub fn clear_all_props(&mut self) {
        self.props.clear();
        self.prop_types.clear();
        self.anything_changed = false;
    }

    fn cull<C>(&mut self, cull_sphere: &mut C)
    where
        C: FnMut(&SphereClass) -> bool,
    {
        for prop in &mut self.props {
            prop.visible = !cull_sphere(&prop.bounds);
        }
    }

    fn find_prop_type(&self, model_name: &str) -> i32 {
        let requested = canonical_model_name(model_name);
        self.prop_types
            .iter()
            .position(|prop_type| canonical_model_name(&prop_type.model_name) == requested)
            .map_or(-1, |index| index as i32)
    }
}

impl Default for W3DPropBuffer {
    fn default() -> Self {
        Self::new()
    }
}

fn canonical_model_name(model_name: &str) -> String {
    model_name.to_ascii_lowercase()
}

fn shroud_rank(status: ObjectShroudStatus) -> u8 {
    status as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn coord(x: f32, y: f32, z: f32) -> Coord3D {
        Coord3D::new(x, y, z)
    }

    fn bounds(radius: f32) -> SphereClass {
        SphereClass::new([1.0, 2.0, 3.0], radius)
    }

    fn buffer_with_model(name: &str) -> W3DPropBuffer {
        let mut buffer = W3DPropBuffer::new();
        buffer.register_model_bounds(name, bounds(4.0));
        buffer
    }

    #[test]
    fn constructor_matches_cpp_initial_state() {
        let buffer = W3DPropBuffer::new();

        assert!(buffer.initialized());
        assert_eq!(buffer.num_props(), 0);
        assert_eq!(buffer.num_prop_types(), 0);
        assert!(!buffer.anything_changed());
        assert_eq!(buffer.xfer_version(), 1);
    }

    #[test]
    fn add_prop_type_requires_registered_model_bounds() {
        let mut buffer = W3DPropBuffer::new();

        assert_eq!(buffer.add_prop_type("TREE01"), -1);

        buffer.register_model_bounds("TREE01", bounds(3.0));
        assert_eq!(buffer.add_prop_type("TREE01"), 0);
        assert_eq!(buffer.prop_types()[0].model_name, "TREE01");
        assert_eq!(buffer.prop_types()[0].bounds, bounds(3.0));
    }

    #[test]
    fn add_prop_reuses_type_case_insensitively_and_translates_bounds() {
        let mut buffer = buffer_with_model("TreeA");

        buffer.add_prop(7, coord(10.0, 20.0, 30.0), 1.25, 2.0, "treea");
        buffer.add_prop(8, coord(-1.0, -2.0, -3.0), 2.25, 0.5, "TREEA");

        assert_eq!(buffer.num_prop_types(), 1);
        assert_eq!(buffer.num_props(), 2);
        assert_eq!(buffer.props()[0].prop_type, 0);
        assert_eq!(buffer.props()[0].shroud_status, ObjectShroudStatus::Invalid);
        assert!(!buffer.props()[0].visible);
        assert_eq!(
            buffer.props()[0].bounds,
            SphereClass::new([11.0, 22.0, 33.0], 4.0)
        );
        assert_eq!(buffer.props()[0].angle, 1.25);
        assert_eq!(buffer.props()[0].scale, 2.0);
    }

    #[test]
    fn add_prop_ignores_missing_model_and_max_prop_overflow() {
        let mut buffer = W3DPropBuffer::new();
        buffer.add_prop(1, coord(0.0, 0.0, 0.0), 0.0, 1.0, "MISSING");
        assert_eq!(buffer.num_props(), 0);

        buffer.register_model_bounds("TREE", bounds(1.0));
        for id in 0..=MAX_PROPS as i32 {
            buffer.add_prop(id, coord(id as f32, 0.0, 0.0), 0.0, 1.0, "TREE");
        }
        assert_eq!(buffer.num_props(), MAX_PROPS);
    }

    #[test]
    fn add_prop_type_preserves_cpp_overflow_return_value() {
        let mut buffer = W3DPropBuffer::new();
        for index in 0..MAX_TYPES {
            let name = format!("MODEL{index}");
            buffer.register_model_bounds(&name, bounds(index as f32 + 1.0));
            assert_eq!(buffer.add_prop_type(&name), index as i32);
        }

        buffer.register_model_bounds("EXTRA", bounds(99.0));
        assert_eq!(buffer.add_prop_type("EXTRA"), 0);
        assert_eq!(buffer.num_prop_types(), MAX_TYPES);
    }

    #[test]
    fn update_prop_position_refreshes_transform_state_and_bounds() {
        let mut buffer = buffer_with_model("TREE");
        buffer.add_prop(5, coord(0.0, 0.0, 0.0), 0.0, 1.0, "TREE");

        assert!(buffer.update_prop_position(5, coord(3.0, 4.0, 5.0), 6.0, 7.0));
        assert_eq!(buffer.props()[0].location.x, 3.0);
        assert_eq!(buffer.props()[0].angle, 6.0);
        assert_eq!(buffer.props()[0].scale, 7.0);
        assert_eq!(
            buffer.props()[0].bounds,
            SphereClass::new([4.0, 6.0, 8.0], 4.0)
        );
        assert!(buffer.anything_changed());
        assert!(!buffer.update_prop_position(99, coord(0.0, 0.0, 0.0), 0.0, 1.0));
    }

    #[test]
    fn remove_prop_preserves_slot_count_and_resets_render_state() {
        let mut buffer = buffer_with_model("TREE");
        buffer.add_prop(5, coord(1.0, 2.0, 3.0), 0.5, 2.0, "TREE");

        buffer.remove_prop(5);

        assert_eq!(buffer.num_props(), 1);
        assert_eq!(buffer.props()[0].id, 5);
        assert!(!buffer.props()[0].render_object_alive);
        assert_eq!(buffer.props()[0].prop_type, -1);
        assert_eq!(buffer.props()[0].location.x, 0.0);
        assert_eq!(buffer.props()[0].bounds, SphereClass::default());
    }

    #[test]
    fn remove_props_for_construction_uses_cpp_cylinder_extents() {
        let mut buffer = buffer_with_model("TREE");
        buffer.add_prop(1, coord(1.0, 0.0, 0.0), 0.0, 1.0, "TREE");
        buffer.add_prop(2, coord(2.0, 0.0, 0.0), 0.0, 1.0, "TREE");
        let mut seen = Vec::new();

        buffer.remove_props_for_construction(|location, geom| {
            seen.push((location.x, geom));
            location.x == 2.0
        });

        assert_eq!(seen.len(), 2);
        assert_eq!(
            seen[0].1,
            ConstructionPropGeometry {
                geometry_type: PropGeometryType::Cylinder,
                is_small: false,
                height: 20.0,
                major_radius: 8.0,
                minor_radius: 8.0,
            }
        );
        assert!(buffer.props()[0].render_object_alive);
        assert!(!buffer.props()[1].render_object_alive);
    }

    #[test]
    fn notify_shroud_changed_matches_partition_manager_presence() {
        let mut buffer = buffer_with_model("TREE");
        buffer.add_prop(1, coord(0.0, 0.0, 0.0), 0.0, 1.0, "TREE");

        buffer.notify_shroud_changed(false);
        assert_eq!(buffer.props()[0].shroud_status, ObjectShroudStatus::Clear);

        buffer.notify_shroud_changed(true);
        assert_eq!(buffer.props()[0].shroud_status, ObjectShroudStatus::Invalid);
    }

    #[test]
    fn draw_props_culls_only_after_full_update_and_filters_shroud() {
        let mut buffer = buffer_with_model("TREE");
        buffer.add_prop(1, coord(1.0, 0.0, 0.0), 0.0, 1.0, "TREE");
        buffer.add_prop(2, coord(2.0, 0.0, 0.0), 0.0, 1.0, "TREE");
        buffer.props[0].visible = true;
        buffer.props[1].visible = true;
        buffer.do_full_update();

        let calls = buffer.draw_props(
            |sphere| sphere.center[0] > 2.0,
            |location| {
                if location.x == 1.0 {
                    ObjectShroudStatus::PartialClear
                } else {
                    ObjectShroudStatus::Shrouded
                }
            },
            true,
            true,
            Some(CellShroudStatus::Shrouded),
        );

        assert_eq!(
            calls,
            vec![PropDrawCall {
                id: 1,
                uses_shroud_material_pass: true,
            }]
        );
    }

    #[test]
    fn draw_props_worldbuilder_path_forces_clear_without_shroud_lookup() {
        let mut buffer = buffer_with_model("TREE");
        buffer.add_prop(1, coord(0.0, 0.0, 0.0), 0.0, 1.0, "TREE");
        buffer.props[0].visible = true;
        let mut shroud_queries = 0;

        let calls = buffer.draw_props(
            |_| false,
            |_| {
                shroud_queries += 1;
                ObjectShroudStatus::Shrouded
            },
            false,
            false,
            Some(CellShroudStatus::Shrouded),
        );

        assert_eq!(shroud_queries, 0);
        assert_eq!(
            calls,
            vec![PropDrawCall {
                id: 1,
                uses_shroud_material_pass: true,
            }]
        );
        assert_eq!(buffer.props()[0].shroud_status, ObjectShroudStatus::Clear);
    }
}
