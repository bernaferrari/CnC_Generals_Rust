//! W3DPropBuffer — draw buffer for all props in a scene.
//!
//! Corresponds to C++ file:
//!   GameEngineDevice/Include/W3DDevice/GameClient/W3DPropBuffer.h
//!
//! C++ uses raw `RenderObjClass*` pointers in TProp and TPropType. This Rust
//! port replaces them with indices into a shared render-object pool, which
//! provides safe aliasing without changing observable behavior.

use std::collections::HashMap;

/// Maximum number of individual props (matches C++ MAX_PROPS).
pub const MAX_PROPS: usize = 4000;
/// Maximum number of distinct prop types (matches C++ MAX_TYPES).
pub const MAX_TYPES: usize = 64;

/// Shroud visibility status for a prop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectShroudStatus {
    /// Not under shroud — fully visible.
    Clear,
    /// Partially shrouded — dimmed.
    Shrouded,
    /// Fully obscured — invisible.
    Fogged,
}

impl Default for ObjectShroudStatus {
    fn default() -> Self {
        ObjectShroudStatus::Clear
    }
}

/// Bounding sphere for culling (center x/y/z + radius).
#[derive(Debug, Clone, Copy, Default)]
pub struct BoundingSphere {
    pub center_x: f32,
    pub center_y: f32,
    pub center_z: f32,
    pub radius: f32,
}

/// Data for a single prop type.
///
/// C++ parity: `TPropType` with `RenderObjClass* m_robj`, `AsciiString m_robjName`,
/// `SphereClass m_bounds`. The raw `m_robj` pointer is replaced by an index into
/// a shared render-object pool held externally by the terrain render system.
#[derive(Debug, Clone)]
pub struct PropTypeEntry {
    /// Index into the terrain render system's render-object pool.
    /// Replaces C++ `RenderObjClass* m_robj`.
    pub render_obj_index: Option<usize>,
    /// Name of the W3D render object (INI model name).
    pub render_obj_name: String,
    /// Bounding sphere for culling the base model.
    pub bounds: BoundingSphere,
}

impl Default for PropTypeEntry {
    fn default() -> Self {
        Self {
            render_obj_index: None,
            render_obj_name: String::new(),
            bounds: BoundingSphere::default(),
        }
    }
}

/// Data for a single placed prop instance.
///
/// C++ parity: `TProp` with `RenderObjClass* m_robj`, `Int id`, `Coord3D location`,
/// `Int propType`, `ObjectShroudStatus ss`, `Bool visible`, `SphereClass bounds`.
/// The raw `m_robj` pointer is replaced by an index.
#[derive(Debug, Clone)]
pub struct PropEntry {
    /// Index into the terrain render system's render-object pool.
    /// Replaces C++ `RenderObjClass* m_robj`.
    pub render_obj_index: Option<usize>,
    /// Unique identifier matching the drawable ID that placed this prop.
    pub id: i32,
    /// World-space location.
    pub location_x: f32,
    pub location_y: f32,
    pub location_z: f32,
    /// Index into `m_prop_types` identifying the kind of prop.
    pub prop_type: i32,
    /// Shroud visibility status.
    pub shroud_status: ObjectShroudStatus,
    /// Whether this prop passed frustum culling this frame.
    pub visible: bool,
    /// Bounding sphere for culling.
    pub bounds: BoundingSphere,
}

impl Default for PropEntry {
    fn default() -> Self {
        Self {
            render_obj_index: None,
            id: 0,
            location_x: 0.0,
            location_y: 0.0,
            location_z: 0.0,
            prop_type: -1,
            shroud_status: ObjectShroudStatus::default(),
            visible: true,
            bounds: BoundingSphere::default(),
        }
    }
}

/// Draw buffer for all props in a scene.
///
/// C++ parity: `W3DPropBuffer : Snapshot` with `TProp m_props[MAX_PROPS]`,
/// `TPropType m_propTypes[MAX_TYPES]`.
///
/// In C++, both `TProp` and `TPropType` hold `RenderObjClass*` raw pointers
/// to the WW3D render objects. Those pointers alias the same `RenderObjClass`
/// instances referenced from the render-obj pool. To avoid unsafe raw-pointer
/// aliasing, this Rust port stores `Option<usize>` indices that the terrain
/// render system resolves to the actual render objects at draw time.
pub struct W3DPropBuffer {
    /// All placed prop instances.
    props: Vec<PropEntry>,
    /// Number of active props.
    num_props: i32,
    /// Whether visibility or sorting changed since last draw.
    anything_changed: bool,
    /// Whether the subsystem has been initialized.
    initialized: bool,
    /// Whether cull needs to be rerun.
    do_cull: bool,
    /// Distinct prop types (shared render-object definitions).
    prop_types: Vec<PropTypeEntry>,
    /// Number of active prop types.
    num_prop_types: i32,
    /// Map from model name to prop-type index for fast lookup.
    prop_type_lookup: HashMap<String, i32>,
}

impl W3DPropBuffer {
    pub fn new() -> Self {
        Self {
            props: Vec::with_capacity(MAX_PROPS),
            num_props: 0,
            anything_changed: false,
            initialized: false,
            do_cull: true,
            prop_types: Vec::with_capacity(MAX_TYPES),
            num_prop_types: 0,
            prop_type_lookup: HashMap::new(),
        }
    }

    /// Add a prop at the given location.
    ///
    /// C++ parity: `void addProp(Int id, Coord3D location, Real angle, Real scale, const AsciiString &modelName)`.
    pub fn add_prop(
        &mut self,
        id: i32,
        loc_x: f32,
        loc_y: f32,
        loc_z: f32,
        angle: f32,
        scale: f32,
        model_name: &str,
    ) {
        if self.num_props as usize >= MAX_PROPS {
            return;
        }

        let prop_type = self.add_prop_type(model_name);

        let entry = PropEntry {
            render_obj_index: if (prop_type as usize) < self.prop_types.len() {
                self.prop_types[prop_type as usize].render_obj_index
            } else {
                None
            },
            id,
            location_x: loc_x,
            location_y: loc_y,
            location_z: loc_z,
            prop_type,
            shroud_status: ObjectShroudStatus::Clear,
            visible: true,
            bounds: BoundingSphere::default(),
        };

        self.props.push(entry);
        self.num_props += 1;
        self.anything_changed = true;
    }

    /// Remove a prop by its ID.
    ///
    /// C++ parity: `void removeProp(Int id)`.
    pub fn remove_prop(&mut self, id: i32) {
        if let Some(pos) = self.props.iter().position(|p| p.id == id) {
            self.props.swap_remove(pos);
            self.num_props -= 1;
            self.anything_changed = true;
        }
    }

    /// Update the position of an existing prop.
    ///
    /// C++ parity: `Bool updatePropPosition(Int id, const Coord3D &location, Real angle, Real scale)`.
    pub fn update_prop_position(
        &mut self,
        id: i32,
        loc_x: f32,
        loc_y: f32,
        loc_z: f32,
        _angle: f32,
        _scale: f32,
    ) -> bool {
        if let Some(prop) = self.props.iter_mut().find(|p| p.id == id) {
            prop.location_x = loc_x;
            prop.location_y = loc_y;
            prop.location_z = loc_z;
            self.anything_changed = true;
            true
        } else {
            false
        }
    }

    /// Let the buffer know that the shroud has changed.
    ///
    /// C++ parity: `void notifyShroudChanged(void)`.
    pub fn notify_shroud_changed(&mut self) {
        self.do_cull = true;
        self.anything_changed = true;
    }

    /// Remove props that overlap a construction area.
    ///
    /// C++ parity: `void removePropsForConstruction(const Coord3D* pos, const GeometryInfo& geom, Real angle)`.
    pub fn remove_props_for_construction(&mut self, cx: f32, cy: f32, radius: f32, _angle: f32) {
        let radius_sq = radius * radius;
        self.props.retain(|prop| {
            let dx = prop.location_x - cx;
            let dy = prop.location_y - cy;
            dx * dx + dy * dy > radius_sq
        });
        self.num_props = self.props.len() as i32;
        self.anything_changed = true;
    }

    /// Add a type of prop. Returns the type index.
    ///
    /// C++ parity: `Int addPropType(const AsciiString &modelName)`.
    pub fn add_prop_type(&mut self, model_name: &str) -> i32 {
        if let Some(&idx) = self.prop_type_lookup.get(model_name) {
            return idx;
        }

        if self.num_prop_types as usize >= MAX_TYPES {
            return 0;
        }

        let idx = self.num_prop_types;
        self.prop_types.push(PropTypeEntry {
            render_obj_index: None,
            render_obj_name: model_name.to_string(),
            bounds: BoundingSphere::default(),
        });
        self.prop_type_lookup
            .insert(model_name.to_ascii_lowercase(), idx);
        self.num_prop_types += 1;
        idx
    }

    /// Set the render-object index for a given prop type.
    ///
    /// This replaces the C++ pattern of assigning a `RenderObjClass*` to
    /// `TPropType::m_robj`. The terrain render system calls this after
    /// loading or resolving the W3D model for a prop type.
    pub fn set_prop_type_render_obj(&mut self, type_index: i32, render_obj_index: usize) {
        if let Some(entry) = self.prop_types.get_mut(type_index as usize) {
            entry.render_obj_index = Some(render_obj_index);
        }
    }

    /// Get the render-object index for a prop type.
    pub fn get_prop_type_render_obj(&self, type_index: i32) -> Option<usize> {
        self.prop_types
            .get(type_index as usize)
            .and_then(|e| e.render_obj_index)
    }

    /// Clear all props.
    ///
    /// C++ parity: `void clearAllProps(void)`.
    pub fn clear_all_props(&mut self) {
        self.props.clear();
        self.num_props = 0;
        self.anything_changed = false;
    }

    /// Get the number of active props.
    pub fn num_props(&self) -> i32 {
        self.num_props
    }

    /// Get the number of prop types.
    pub fn num_prop_types(&self) -> i32 {
        self.num_prop_types
    }

    /// Get a prop entry by index.
    pub fn get_prop(&self, index: usize) -> Option<&PropEntry> {
        self.props.get(index)
    }

    /// Get a mutable prop entry by index.
    pub fn get_prop_mut(&mut self, index: usize) -> Option<&mut PropEntry> {
        self.props.get_mut(index)
    }

    /// Get a prop type entry by index.
    pub fn get_prop_type(&self, index: usize) -> Option<&PropTypeEntry> {
        self.prop_types.get(index)
    }

    /// Iterate over all props.
    pub fn props(&self) -> impl Iterator<Item = &PropEntry> {
        self.props.iter()
    }

    /// Iterate over all prop types.
    pub fn prop_types(&self) -> impl Iterator<Item = &PropTypeEntry> {
        self.prop_types.iter()
    }

    /// Mark that a full cull/update is needed.
    ///
    /// C++ parity: `void doFullUpdate(void) { m_doCull = true; }`.
    pub fn do_full_update(&mut self) {
        self.do_cull = true;
    }

    /// CRC for save/load (Xfer).
    pub fn crc(&self) -> u32 {
        0
    }
}

impl Default for W3DPropBuffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prop_buffer_add_and_remove() {
        let mut buf = W3DPropBuffer::new();
        buf.add_prop(1, 10.0, 20.0, 0.0, 0.0, 1.0, "PTPine01");
        buf.add_prop(2, 30.0, 40.0, 0.0, 0.0, 1.0, "PTPine02");
        assert_eq!(buf.num_props(), 2);

        buf.remove_prop(1);
        assert_eq!(buf.num_props(), 1);
        assert_eq!(buf.get_prop(0).unwrap().id, 2);
    }

    #[test]
    fn test_prop_buffer_type_dedup() {
        let mut buf = W3DPropBuffer::new();
        let t1 = buf.add_prop_type("PTPine01");
        let t2 = buf.add_prop_type("PTPine01");
        assert_eq!(t1, t2);
        assert_eq!(buf.num_prop_types(), 1);

        let t3 = buf.add_prop_type("PTBush02");
        assert_ne!(t1, t3);
        assert_eq!(buf.num_prop_types(), 2);
    }

    #[test]
    fn test_prop_buffer_update_position() {
        let mut buf = W3DPropBuffer::new();
        buf.add_prop(10, 1.0, 2.0, 3.0, 0.0, 1.0, "Tree");

        assert!(buf.update_prop_position(10, 5.0, 6.0, 7.0, 0.0, 1.0));
        let prop = buf.get_prop(0).unwrap();
        assert_eq!(prop.location_x, 5.0);
        assert_eq!(prop.location_y, 6.0);
        assert_eq!(prop.location_z, 7.0);

        assert!(!buf.update_prop_position(999, 0.0, 0.0, 0.0, 0.0, 1.0));
    }

    #[test]
    fn test_prop_buffer_clear() {
        let mut buf = W3DPropBuffer::new();
        buf.add_prop(1, 0.0, 0.0, 0.0, 0.0, 1.0, "Tree");
        buf.add_prop(2, 1.0, 1.0, 0.0, 0.0, 1.0, "Tree");
        assert_eq!(buf.num_props(), 2);

        buf.clear_all_props();
        assert_eq!(buf.num_props(), 0);
    }

    #[test]
    fn test_prop_buffer_remove_for_construction() {
        let mut buf = W3DPropBuffer::new();
        buf.add_prop(1, 0.0, 0.0, 0.0, 0.0, 1.0, "Tree");
        buf.add_prop(2, 100.0, 100.0, 0.0, 0.0, 1.0, "Tree");
        buf.add_prop(3, 5.0, 5.0, 0.0, 0.0, 1.0, "Tree");

        buf.remove_props_for_construction(0.0, 0.0, 10.0, 0.0);
        assert_eq!(buf.num_props(), 1);
        assert_eq!(buf.get_prop(0).unwrap().id, 2);
    }

    #[test]
    fn test_prop_type_render_obj_index() {
        let mut buf = W3DPropBuffer::new();
        let t = buf.add_prop_type("Bush");
        assert!(buf.get_prop_type_render_obj(t).is_none());

        buf.set_prop_type_render_obj(t, 42);
        assert_eq!(buf.get_prop_type_render_obj(t), Some(42));
    }

    #[test]
    fn test_prop_buffer_max_props_limit() {
        let mut buf = W3DPropBuffer::new();
        for i in 0..=MAX_PROPS {
            buf.add_prop(i as i32, 0.0, 0.0, 0.0, 0.0, 1.0, "Tree");
        }
        assert_eq!(buf.num_props(), MAX_PROPS as i32);
    }
}
