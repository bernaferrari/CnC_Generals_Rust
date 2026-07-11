use crate::assets::{AssetManager, Prototype, RenderObj};
use glam::{Mat4, Vec3};
use std::any::Any;
use std::fmt::{self, Debug};
use ww3d_core::*;

/// Serialized AABTree data captured from the W3D file.
#[derive(Debug, Clone)]
pub struct AABTreePrototype {
    pub header: W3dAABTreeHeader,
    pub nodes: Vec<W3dAABTreeNode>,
    pub poly_indices: Vec<u32>,
}

/// LOD entry describing a render object and its distance range
#[derive(Debug, Clone)]
pub struct LodEntry {
    pub render_obj_name: String,
    pub lod_min: f32,
    pub lod_max: f32,
}

/// Prototype capturing a classic WW3D LOD model definition
#[derive(Debug)]
pub struct LodModelPrototype {
    pub name: String,
    pub version: u32,
    pub lods: Vec<LodEntry>,
}

/// Placeholder entry for collection proxies
#[derive(Debug, Clone)]
pub struct CollectionPlaceholder {
    pub name: String,
    pub transform: Mat4,
}

/// Transform node entry for external assets instanced by a collection
#[derive(Debug, Clone)]
pub struct CollectionTransformNode {
    pub name: String,
    pub transform: Mat4,
}

/// Collection prototype mirroring the C++ CollectionDefClass data
#[derive(Debug)]
pub struct CollectionPrototype {
    pub name: String,
    pub version: u32,
    pub object_names: Vec<String>,
    pub placeholders: Vec<CollectionPlaceholder>,
    pub transform_nodes: Vec<CollectionTransformNode>,
    pub snap_points: Vec<W3dVectorStruct>,
}

pub struct CollectionMember {
    name: String,
    local_transform: Mat4,
    object: Option<Box<dyn RenderObj>>,
}

impl fmt::Debug for CollectionMember {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CollectionMember")
            .field("name", &self.name)
            .field("has_object", &self.object.is_some())
            .finish()
    }
}

impl CollectionMember {
    fn new(name: String, local_transform: Mat4, object: Option<Box<dyn RenderObj>>) -> Self {
        Self {
            name,
            local_transform,
            object,
        }
    }

    fn set_world_transform(&mut self, world: Mat4) {
        if let Some(obj) = self.object.as_mut() {
            obj.set_transform(world * self.local_transform);
        }
    }

    fn render(&self) {
        if let Some(obj) = self.object.as_ref() {
            obj.render();
        }
    }

    pub fn object(&self) -> Option<&dyn RenderObj> {
        self.object.as_deref()
    }

    pub fn object_mut(&mut self) -> Option<&mut Box<dyn RenderObj>> {
        self.object.as_mut()
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn local_transform(&self) -> &Mat4 {
        &self.local_transform
    }
}

/// Runtime instance mirroring the C++ `CollectionClass`.
pub struct CollectionInstance {
    name: String,
    world_transform: Mat4,
    members: Vec<CollectionMember>,
    placeholders: Vec<CollectionPlaceholder>,
    snap_points: Vec<W3dVectorStruct>,
}

impl fmt::Debug for CollectionInstance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CollectionInstance")
            .field("name", &self.name)
            .field("member_count", &self.members.len())
            .field("placeholder_count", &self.placeholders.len())
            .finish()
    }
}

impl CollectionInstance {
    fn new(prototype: &CollectionPrototype, assets: &AssetManager) -> Self {
        use std::collections::HashMap;

        let mut transform_map: HashMap<&str, Mat4> = HashMap::new();
        for node in &prototype.transform_nodes {
            transform_map.insert(node.name.as_str(), node.transform);
        }

        let mut members = Vec::with_capacity(prototype.object_names.len());
        for object_name in &prototype.object_names {
            let instance = assets.create_instance(object_name);
            if instance.is_none() {
                println!(
                    "Collection '{}' missing referenced object '{}'",
                    prototype.name, object_name
                );
            }
            let local_transform = transform_map
                .get(object_name.as_str())
                .copied()
                .unwrap_or(Mat4::IDENTITY);
            members.push(CollectionMember::new(
                object_name.clone(),
                local_transform,
                instance,
            ));
        }

        let mut instance = Self {
            name: prototype.name.clone(),
            world_transform: Mat4::IDENTITY,
            members,
            placeholders: prototype.placeholders.clone(),
            snap_points: prototype.snap_points.clone(),
        };
        instance.propagate_transforms();
        instance
    }

    fn propagate_transforms(&mut self) {
        let world = self.world_transform;
        for member in &mut self.members {
            member.set_world_transform(world);
        }
    }

    pub fn members(&self) -> &[CollectionMember] {
        &self.members
    }

    pub fn members_mut(&mut self) -> &mut [CollectionMember] {
        &mut self.members
    }

    pub fn member(&self, name: &str) -> Option<&CollectionMember> {
        self.members.iter().find(|member| member.name == name)
    }

    pub fn member_mut(&mut self, name: &str) -> Option<&mut CollectionMember> {
        self.members.iter_mut().find(|member| member.name == name)
    }

    pub fn placeholders(&self) -> &[CollectionPlaceholder] {
        &self.placeholders
    }

    pub fn placeholder(&self, name: &str) -> Option<&CollectionPlaceholder> {
        self.placeholders.iter().find(|ph| ph.name == name)
    }

    pub fn snap_points(&self) -> &[W3dVectorStruct] {
        &self.snap_points
    }

    pub fn placeholder_world_transform(&self, name: &str) -> Option<Mat4> {
        self.placeholder(name)
            .map(|ph| self.world_transform * ph.transform)
    }

    pub fn world_snap_points(&self) -> Vec<Vec3> {
        self.snap_points
            .iter()
            .map(|pt| {
                let local = Vec3::new(pt.x, pt.y, pt.z);
                self.world_transform.transform_point3(local)
            })
            .collect()
    }
}

/// Prototype for hierarchical LOD definitions
#[derive(Debug, Clone)]
pub struct HlodPrototype {
    pub name: String,
    pub hierarchy_name: String,
    pub version: u32,
    pub lods: Vec<HlodLodEntry>,
    pub aggregates: Vec<HlodAggregateEntry>,
    pub proxy_entries: Vec<HlodProxyEntry>,
}

#[derive(Debug, Clone)]
pub struct HlodProxyEntry {
    pub name: String,
    pub bone_index: u32,
}

#[derive(Debug, Clone)]
pub struct HlodLodEntry {
    pub max_screen_size: f32,
    pub models: Vec<HlodSubObject>,
}

#[derive(Debug, Clone)]
pub struct HlodSubObject {
    pub name: String,
    pub bone_index: u32,
}

#[derive(Debug, Clone)]
pub struct HlodAggregateEntry {
    pub max_screen_size: f32,
    pub models: Vec<HlodSubObject>,
}

/// Runtime helper describing a render object bound to a specific bone index.
pub struct HlodModelInstance {
    pub name: String,
    pub bone_index: i32,
    pub object: Option<Box<dyn RenderObj>>,
}

impl Clone for HlodModelInstance {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            bone_index: self.bone_index,
            object: self.object.as_ref().map(|obj| obj.clone_box()),
        }
    }
}

impl HlodModelInstance {
    pub fn new(name: String, bone_index: i32, object: Option<Box<dyn RenderObj>>) -> Self {
        Self {
            name,
            bone_index,
            object,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn bone_index(&self) -> i32 {
        self.bone_index
    }

    pub fn object(&self) -> Option<&dyn RenderObj> {
        self.object.as_deref()
    }

    pub fn object_mut(&mut self) -> Option<&mut Box<dyn RenderObj>> {
        self.object.as_mut()
    }

    fn render(&self) {
        if let Some(obj) = self.object.as_ref() {
            obj.render();
        }
    }

    fn set_transform(&mut self, transform: Mat4) {
        if let Some(obj) = self.object.as_mut() {
            obj.set_transform(transform);
        }
    }
}

impl fmt::Debug for HlodModelInstance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HlodModelInstance")
            .field("name", &self.name)
            .field("bone_index", &self.bone_index)
            .field("has_object", &self.object.is_some())
            .finish()
    }
}

/// Collection of models forming a single LOD level.
#[derive(Clone)]
pub struct HlodLodLevel {
    pub max_screen_size: f32,
    pub models: Vec<HlodModelInstance>,
}

impl HlodLodLevel {
    pub fn new(max_screen_size: f32) -> Self {
        Self {
            max_screen_size,
            models: Vec::new(),
        }
    }

    fn render(&self) {
        for model in &self.models {
            model.render();
        }
    }

    fn set_transform(&mut self, transform: Mat4) {
        for model in &mut self.models {
            model.set_transform(transform);
        }
    }

    pub fn models(&self) -> &[HlodModelInstance] {
        &self.models
    }

    pub fn models_mut(&mut self) -> &mut [HlodModelInstance] {
        &mut self.models
    }

    pub fn is_visible(&self, screen_size: f32) -> bool {
        self.max_screen_size <= 1e-4 || screen_size <= self.max_screen_size
    }
}

impl fmt::Debug for HlodLodLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HlodLodLevel")
            .field("max_screen_size", &self.max_screen_size)
            .field("model_count", &self.models.len())
            .finish()
    }
}

/// Additional models attached to an HLOD (aggregate attachments).
#[derive(Clone)]
pub struct HlodAggregateGroup {
    pub max_screen_size: f32,
    pub models: Vec<HlodModelInstance>,
}

impl HlodAggregateGroup {
    pub fn new(max_screen_size: f32) -> Self {
        Self {
            max_screen_size,
            models: Vec::new(),
        }
    }

    fn render(&self) {
        for model in &self.models {
            model.render();
        }
    }

    fn set_transform(&mut self, transform: Mat4) {
        for model in &mut self.models {
            model.set_transform(transform);
        }
    }

    pub fn models(&self) -> &[HlodModelInstance] {
        &self.models
    }

    pub fn models_mut(&mut self) -> &mut [HlodModelInstance] {
        &mut self.models
    }

    pub fn is_visible(&self, screen_size: f32) -> bool {
        self.max_screen_size <= 1e-4 || screen_size <= self.max_screen_size
    }
}

impl fmt::Debug for HlodAggregateGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HlodAggregateGroup")
            .field("max_screen_size", &self.max_screen_size)
            .field("model_count", &self.models.len())
            .finish()
    }
}

pub struct HlodInstance {
    name: String,
    hierarchy_name: String,
    transform: Mat4,
    lods: Vec<HlodLodLevel>,
    current_lod: usize,
    aggregates: Vec<HlodAggregateGroup>,
    proxies: Vec<HlodProxyEntry>,
}

impl HlodInstance {
    fn new(
        name: String,
        hierarchy_name: String,
        lods: Vec<HlodLodLevel>,
        aggregates: Vec<HlodAggregateGroup>,
        proxies: Vec<HlodProxyEntry>,
    ) -> Self {
        let mut instance = Self {
            name,
            hierarchy_name,
            transform: Mat4::IDENTITY,
            lods,
            current_lod: 0,
            aggregates,
            proxies,
        };
        instance.propagate_transform();
        instance
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn hierarchy_name(&self) -> &str {
        &self.hierarchy_name
    }

    pub fn lod_count(&self) -> usize {
        self.lods.len()
    }

    pub fn current_lod_index(&self) -> usize {
        if self.lods.is_empty() {
            0
        } else {
            self.current_lod.min(self.lods.len() - 1)
        }
    }

    pub fn current_lod(&self) -> Option<&HlodLodLevel> {
        self.lods.get(self.current_lod_index())
    }

    pub fn current_lod_mut(&mut self) -> Option<&mut HlodLodLevel> {
        if self.lods.is_empty() {
            None
        } else {
            let index = self.current_lod_index();
            self.lods.get_mut(index)
        }
    }

    pub fn lods(&self) -> &[HlodLodLevel] {
        &self.lods
    }

    pub fn aggregates(&self) -> &[HlodAggregateGroup] {
        &self.aggregates
    }

    pub fn proxies(&self) -> &[HlodProxyEntry] {
        &self.proxies
    }

    pub fn select_lod_by_screen_size(&mut self, screen_size: f32) {
        if self.lods.is_empty() {
            self.current_lod = 0;
            return;
        }

        let mut selected = self.lods.len().saturating_sub(1);
        for (idx, level) in self.lods.iter().enumerate() {
            let max = level.max_screen_size;
            if max <= 1e-4 || screen_size <= max {
                selected = idx;
                break;
            }
        }
        self.current_lod = selected;
    }

    pub fn set_lod_level(&mut self, index: usize) {
        if !self.lods.is_empty() {
            self.current_lod = index.min(self.lods.len() - 1);
        }
    }

    pub fn visible_aggregates<'a>(
        &'a self,
        screen_size: f32,
    ) -> impl Iterator<Item = &'a HlodAggregateGroup> + 'a {
        self.aggregates
            .iter()
            .filter(move |group| group.is_visible(screen_size))
    }

    pub fn transform(&self) -> &Mat4 {
        &self.transform
    }

    fn propagate_transform(&mut self) {
        let transform = self.transform;
        for lod in &mut self.lods {
            lod.set_transform(transform);
        }
        for group in &mut self.aggregates {
            group.set_transform(transform);
        }
    }
}

impl fmt::Debug for HlodInstance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HlodInstance")
            .field("name", &self.name)
            .field("hierarchy", &self.hierarchy_name)
            .field("lod_count", &self.lods.len())
            .field("aggregate_count", &self.aggregates.len())
            .field("proxy_count", &self.proxies.len())
            .finish()
    }
}

impl RenderObj for HlodInstance {
    fn render(&self) {
        if let Some(lod) = self.lods.get(self.current_lod_index()) {
            lod.render();
        }
        for group in &self.aggregates {
            group.render();
        }
    }

    fn get_name(&self) -> &str {
        &self.name
    }

    fn set_transform(&mut self, transform: Mat4) {
        self.transform = transform;
        self.propagate_transform();
    }

    fn get_transform(&self) -> &Mat4 {
        &self.transform
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn RenderObj> {
        Box::new(Self {
            name: self.name.clone(),
            hierarchy_name: self.hierarchy_name.clone(),
            transform: self.transform,
            lods: self.lods.to_vec(),
            current_lod: self.current_lod,
            aggregates: self.aggregates.to_vec(),
            proxies: self.proxies.clone(),
        })
    }
}

impl Prototype for LodModelPrototype {
    fn create_instance(&self, _assets: &AssetManager) -> Option<Box<dyn RenderObj>> {
        // LOD models act as blueprints for higher level systems; runtime renderer consumes them.
        None
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Prototype for CollectionPrototype {
    fn create_instance(&self, assets: &AssetManager) -> Option<Box<dyn RenderObj>> {
        Some(Box::new(CollectionInstance::new(self, assets)))
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn class_id(&self) -> Option<RenderObjClassId> {
        Some(RenderObjClassId::Collection)
    }
}

impl RenderObj for CollectionInstance {
    fn render(&self) {
        for member in &self.members {
            member.render();
        }
    }

    fn get_name(&self) -> &str {
        &self.name
    }

    fn set_transform(&mut self, transform: Mat4) {
        self.world_transform = transform;
        self.propagate_transforms();
    }

    fn get_transform(&self) -> &Mat4 {
        &self.world_transform
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn RenderObj> {
        Box::new(Self {
            name: self.name.clone(),
            world_transform: self.world_transform,
            members: self
                .members
                .iter()
                .map(|member| {
                    CollectionMember::new(
                        member.name.clone(),
                        *member.local_transform(),
                        member.object.as_ref().map(|obj| obj.clone_box()),
                    )
                })
                .collect(),
            placeholders: self.placeholders.clone(),
            snap_points: self.snap_points.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::{AssetManager, Prototype, RenderObj};
    use std::any::Any;

    #[derive(Debug)]
    struct DummyRenderObj {
        name: String,
        transform: Mat4,
    }

    impl DummyRenderObj {
        fn new(name: String) -> Self {
            Self {
                name,
                transform: Mat4::IDENTITY,
            }
        }
    }

    impl RenderObj for DummyRenderObj {
        fn render(&self) {}

        fn get_name(&self) -> &str {
            &self.name
        }

        fn set_transform(&mut self, transform: Mat4) {
            self.transform = transform;
        }

        fn get_transform(&self) -> &Mat4 {
            &self.transform
        }

        fn as_any(&self) -> &dyn Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }

        fn clone_box(&self) -> Box<dyn RenderObj> {
            Box::new(DummyRenderObj {
                name: self.name.clone(),
                transform: self.transform,
            })
        }
    }

    #[derive(Debug)]
    struct DummyPrototype {
        name: String,
    }

    impl DummyPrototype {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
            }
        }
    }

    impl Prototype for DummyPrototype {
        fn create_instance(&self, _assets: &AssetManager) -> Option<Box<dyn RenderObj>> {
            Some(Box::new(DummyRenderObj::new(self.name.clone())))
        }

        fn name(&self) -> &str {
            &self.name
        }

        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    #[test]
    fn collection_instance_instantiates_and_propagates_transforms() {
        let mut assets = AssetManager::new();
        assets.add_prototype("OBJ_A".into(), Box::new(DummyPrototype::new("OBJ_A")));
        assets.add_prototype("OBJ_B".into(), Box::new(DummyPrototype::new("OBJ_B")));

        let prototype = CollectionPrototype {
            name: "TEST_COLLECTION".into(),
            version: 1,
            object_names: vec!["OBJ_A".into(), "OBJ_B".into()],
            placeholders: vec![CollectionPlaceholder {
                name: "SpawnPoint".into(),
                transform: Mat4::from_translation(Vec3::new(1.0, 0.0, 0.0)),
            }],
            transform_nodes: vec![CollectionTransformNode {
                name: "OBJ_B".into(),
                transform: Mat4::from_scale(Vec3::splat(2.0)),
            }],
            snap_points: vec![W3dVectorStruct {
                x: 0.0,
                y: 1.0,
                z: 2.0,
            }],
        };

        let mut instance = CollectionInstance::new(&prototype, &assets);
        assert_eq!(instance.members().len(), 2);
        assert!(instance.placeholder("SpawnPoint").is_some());

        // Ensure transforms propagate to members
        let world = Mat4::from_translation(Vec3::new(0.0, 2.0, 0.0));
        instance.set_transform(world);

        let member_b = instance.member_mut("OBJ_B").expect("member B");
        let dummy = member_b
            .object_mut()
            .and_then(|obj| obj.as_any_mut().downcast_mut::<DummyRenderObj>())
            .expect("dummy render obj");
        assert_eq!(dummy.transform, world * Mat4::from_scale(Vec3::splat(2.0)));

        let spawn_xform = instance
            .placeholder_world_transform("SpawnPoint")
            .expect("world spawn");
        assert_eq!(
            spawn_xform,
            world * Mat4::from_translation(Vec3::new(1.0, 0.0, 0.0))
        );

        let snap_points = instance.world_snap_points();
        assert_eq!(snap_points.len(), 1);
        assert_eq!(snap_points[0], Vec3::new(0.0, 3.0, 2.0));
    }

    #[test]
    fn hierarchy_prototype_computes_bind_transforms() {
        let mut hierarchy = HierarchyPrototype::new("TestHierarchy".into());
        hierarchy.pivots = vec![
            make_pivot("ROOT", -1, [0.0, 0.0, 0.0]),
            make_pivot("BONE", 0, [1.0, 0.0, 0.0]),
        ];
        hierarchy.num_pivots = hierarchy.pivots.len() as u32;

        hierarchy.recompute_bind_transforms();

        assert_eq!(hierarchy.bind_transforms.len(), 2);
        assert_eq!(hierarchy.inverse_bind_transforms.len(), 2);
        assert_eq!(hierarchy.bind_transforms[0], Mat4::IDENTITY);
        assert_eq!(
            hierarchy.bind_transforms[1],
            Mat4::from_translation(Vec3::new(1.0, 0.0, 0.0))
        );
        let inverse = hierarchy.inverse_bind_transforms[1];
        let expected_inverse = Mat4::from_translation(Vec3::new(-1.0, 0.0, 0.0));
        assert_eq!(inverse, expected_inverse);
    }

    fn make_pivot(name: &str, parent_idx: i32, translation: [f32; 3]) -> W3dPivotStruct {
        let mut name_bytes = [0u8; 16];
        let raw = name.as_bytes();
        let len = raw.len().min(16);
        name_bytes[..len].copy_from_slice(&raw[..len]);
        W3dPivotStruct {
            name: name_bytes,
            parent_idx,
            translation: W3dVectorStruct {
                x: translation[0],
                y: translation[1],
                z: translation[2],
            },
            euler_angles: W3dVectorStruct {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
        }
    }
}

impl Prototype for HlodPrototype {
    fn create_instance(&self, assets: &AssetManager) -> Option<Box<dyn RenderObj>> {
        if self.lods.is_empty() {
            println!("HLOD '{}' has no LOD entries", self.name);
            return None;
        }

        let mut lod_levels = Vec::with_capacity(self.lods.len());
        for lod in &self.lods {
            let mut level = HlodLodLevel::new(lod.max_screen_size);
            for model in &lod.models {
                let object = assets.create_instance(&model.name);
                if object.is_none() {
                    println!("HLOD '{}' missing sub-object '{}'", self.name, model.name);
                }
                level.models.push(HlodModelInstance::new(
                    model.name.clone(),
                    model.bone_index as i32,
                    object,
                ));
            }
            lod_levels.push(level);
        }

        let mut aggregate_groups = Vec::with_capacity(self.aggregates.len());
        for aggregate in &self.aggregates {
            let mut group = HlodAggregateGroup::new(aggregate.max_screen_size);
            for model in &aggregate.models {
                let object = assets.create_instance(&model.name);
                if object.is_none() {
                    println!("HLOD '{}' missing aggregate '{}'", self.name, model.name);
                }
                group.models.push(HlodModelInstance::new(
                    model.name.clone(),
                    model.bone_index as i32,
                    object,
                ));
            }
            aggregate_groups.push(group);
        }

        let proxies = self.proxy_entries.clone();

        let instance = HlodInstance::new(
            self.name.clone(),
            self.hierarchy_name.clone(),
            lod_levels,
            aggregate_groups,
            proxies,
        );

        Some(Box::new(instance))
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn class_id(&self) -> Option<RenderObjClassId> {
        Some(RenderObjClassId::Hlod)
    }
}

/// Mesh prototype containing shared mesh data
#[derive(Debug)]
pub struct MeshPrototype {
    pub name: String,
    pub header: Option<W3dMeshHeader3Struct>,
    pub vertices: Vec<W3dVectorStruct>,
    pub normals: Vec<W3dVectorStruct>,
    pub triangles: Vec<W3dTriangleStruct>,
    pub material_info: Option<W3dMaterialInfoStruct>,
    pub shaders: Vec<W3dShaderStruct>,
    pub vertex_materials: Vec<W3dVertexMaterialStruct>,
    pub vertex_material_names: Vec<W3dVertexMaterialNameStruct>,
    pub textures: Vec<W3dTextureStruct>,
    pub vertex_influences: Option<Vec<W3dVertInfStruct>>, // Optional skeletal weights
    pub vertex_shade_indices: Option<Vec<u32>>,           // Optional per-vertex shade indices
    pub passes: Vec<MaterialPassInfo>,                    // Material passes
    pub stage_texcoords: Vec<Vec<W3dTexCoordStruct>>,     // stage -> texcoords array
    pub per_face_texcoord_ids: Vec<Vec<[u32; 3]>>,        // stage -> per-face texcoord ids
    // Per-pass mappings
    pub per_pass_vertex_material_ids: Vec<Vec<u32>>, // pass -> vertex -> vm id
    pub per_pass_shader_ids: Vec<Vec<u32>>,          // pass -> polygon -> shader id
    pub per_pass_stage_texture_ids: Vec<Vec<Vec<u32>>>, // pass -> stage -> ids per poly or single
    pub per_pass_dcg_colors: Vec<Vec<W3dRGBAStruct>>, // pass -> per-vertex colors (DCG)
    pub per_pass_dig_colors: Vec<Vec<W3dRGBAStruct>>, // pass -> per-vertex colors (DIG)
    pub aabtree: Option<AABTreePrototype>,
    pub vertex_mapper_configs: Vec<VertexMapperConfig>,
}

/// Minimal pass info mirror for assembly
#[derive(Debug, Clone)]
pub struct MaterialPassInfo {
    pub vm_id: u32,
    pub shader_id: u32,
    pub texture_count: u32,
}

impl MeshPrototype {
    pub fn new(name: String) -> Self {
        Self {
            name,
            header: None,
            vertices: Vec::new(),
            normals: Vec::new(),
            triangles: Vec::new(),
            material_info: None,
            shaders: Vec::new(),
            vertex_materials: Vec::new(),
            vertex_material_names: Vec::new(),
            textures: Vec::new(),
            vertex_influences: None,
            vertex_shade_indices: None,
            passes: Vec::new(),
            stage_texcoords: Vec::new(),
            per_face_texcoord_ids: Vec::new(),
            per_pass_vertex_material_ids: Vec::new(),
            per_pass_shader_ids: Vec::new(),
            per_pass_stage_texture_ids: Vec::new(),
            per_pass_dcg_colors: Vec::new(),
            per_pass_dig_colors: Vec::new(),
            aabtree: None,
            vertex_mapper_configs: Vec::new(),
        }
    }
}

/// Per-vertex-material mapper configuration extracted from W3D data.
#[derive(Debug, Clone, Default)]
pub struct VertexMapperConfig {
    pub stage0: Option<MapperDefinition>,
    pub stage1: Option<MapperDefinition>,
}

/// Mapper definition (type id + packed arguments).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MapperDefinition {
    pub mapper_type: u32,
    pub args: [i32; 4],
    pub float_args: [f32; 4],
}

impl Prototype for MeshPrototype {
    fn create_instance(&self, _assets: &AssetManager) -> Option<Box<dyn RenderObj>> {
        Some(Box::new(MeshInstance::new(self)))
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn class_id(&self) -> Option<RenderObjClassId> {
        Some(RenderObjClassId::Mesh)
    }
}

/// Mesh instance that owns its data
#[derive(Debug)]
pub struct MeshInstance {
    name: String,
    transform: Mat4,
    // Minimal runtime link to geometry; can be expanded to GPU resources later
    pub vertices: Vec<W3dVectorStruct>,
    pub normals: Vec<W3dVectorStruct>,
}

impl MeshInstance {
    pub fn new(prototype: &MeshPrototype) -> Self {
        Self {
            name: prototype.name.clone(),
            transform: Mat4::IDENTITY,
            vertices: prototype.vertices.clone(),
            normals: prototype.normals.clone(),
        }
    }
}

impl RenderObj for MeshInstance {
    fn render(&self) {
        // Note: Actual WGPU rendering is handled by the renderer backend.
        // This trait method is called during scene traversal, and the renderer
        // extracts vertex/index data for GPU upload and draw call submission.
        // C++ equivalent: RenderObjClass::Render dispatches to DX8 device
        // (Rendering implementation is in wgpu_renderer module)
    }

    fn get_name(&self) -> &str {
        &self.name
    }

    fn set_transform(&mut self, transform: Mat4) {
        self.transform = transform;
    }

    fn get_transform(&self) -> &Mat4 {
        &self.transform
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn RenderObj> {
        Box::new(Self {
            name: self.name.clone(),
            transform: self.transform,
            vertices: self.vertices.clone(),
            normals: self.normals.clone(),
        })
    }
}

/// Hierarchy prototype containing skeleton data
#[derive(Debug)]
pub struct HierarchyPrototype {
    pub name: String,
    pub pivots: Vec<W3dPivotStruct>,
    pub num_pivots: u32,
    pub bind_transforms: Vec<Mat4>,
    pub inverse_bind_transforms: Vec<Mat4>,
}

impl HierarchyPrototype {
    pub fn new(name: String) -> Self {
        Self {
            name,
            pivots: Vec::new(),
            num_pivots: 0,
            bind_transforms: Vec::new(),
            inverse_bind_transforms: Vec::new(),
        }
    }

    /// Recompute bind and inverse bind transforms from the stored pivots.
    pub fn recompute_bind_transforms(&mut self) {
        let count = self.pivots.len();
        self.bind_transforms = vec![Mat4::IDENTITY; count];
        self.inverse_bind_transforms = vec![Mat4::IDENTITY; count];

        for (index, pivot) in self.pivots.iter().enumerate() {
            let parent_idx = pivot.parent_idx;
            let base = pivot.base_transform();

            let world = if index == 0 && parent_idx < 0 {
                // Root pivot mirrors the C++ behaviour: identity root transform.
                Mat4::IDENTITY
            } else if parent_idx >= 0 && (parent_idx as usize) < index {
                self.bind_transforms[parent_idx as usize] * base
            } else {
                base
            };

            self.bind_transforms[index] = world;

            let inverse = world.inverse();
            self.inverse_bind_transforms[index] = if inverse.is_finite() {
                inverse
            } else {
                Mat4::IDENTITY
            };
        }
    }
}

impl Prototype for HierarchyPrototype {
    fn create_instance(&self, _assets: &AssetManager) -> Option<Box<dyn RenderObj>> {
        Some(Box::new(HierarchyInstance::new(self)))
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Hierarchy instance
#[derive(Debug)]
pub struct HierarchyInstance {
    name: String,
    transform: Mat4,
}

impl HierarchyInstance {
    pub fn new(prototype: &HierarchyPrototype) -> Self {
        Self {
            name: prototype.name.clone(),
            transform: Mat4::IDENTITY,
        }
    }
}

impl RenderObj for HierarchyInstance {
    fn render(&self) {
        // Hierarchies don't render directly
        println!("Hierarchy: {}", self.name);
    }

    fn get_name(&self) -> &str {
        &self.name
    }

    fn set_transform(&mut self, transform: Mat4) {
        self.transform = transform;
    }

    fn get_transform(&self) -> &Mat4 {
        &self.transform
    }
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn RenderObj> {
        Box::new(Self {
            name: self.name.clone(),
            transform: self.transform,
        })
    }
}

/// Raw channel payload decoded from a classic W3D animation chunk.
#[derive(Clone, Debug)]
pub struct AnimationChannelData {
    pub first_frame: u16,
    pub last_frame: u16,
    pub vector_len: u16,
    pub flags: u16,
    pub pivot: u16,
    pub data: Vec<f32>,
}

impl AnimationChannelData {
    pub fn frame_count(&self) -> usize {
        if self.last_frame >= self.first_frame {
            (self.last_frame - self.first_frame + 1) as usize
        } else {
            0
        }
    }

    pub fn component_count(&self) -> usize {
        self.frame_count() * self.vector_len as usize
    }
}

/// Animation prototype containing animation data
#[derive(Debug)]
pub struct AnimationPrototype {
    pub name: String,
    pub hierarchy_name: String,
    pub num_frames: u32,
    pub frame_rate: u32,
    pub channels: Vec<AnimationChannelData>,
}

impl AnimationPrototype {
    pub fn new(name: String, hierarchy_name: String) -> Self {
        Self {
            name,
            hierarchy_name,
            num_frames: 0,
            frame_rate: 30,
            channels: Vec::new(),
        }
    }
}

impl Prototype for AnimationPrototype {
    fn create_instance(&self, _assets: &AssetManager) -> Option<Box<dyn RenderObj>> {
        // Animations don't create render objects directly
        None
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// HModel prototype for hierarchical models
#[derive(Debug, Clone)]
pub struct HModelNodeLink {
    pub render_obj_name: String,
    pub pivot_idx: u32,
}

#[derive(Debug, Clone)]
pub struct HModelPrototype {
    pub name: String,
    pub hierarchy_name: String,
    pub nodes: Vec<HModelNodeLink>,
}

impl HModelPrototype {
    pub fn new(name: String, hierarchy_name: String) -> Self {
        Self {
            name,
            hierarchy_name,
            nodes: Vec::new(),
        }
    }
}

impl Prototype for HModelPrototype {
    fn create_instance(&self, _assets: &AssetManager) -> Option<Box<dyn RenderObj>> {
        Some(Box::new(HModelInstance::new(self)))
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn class_id(&self) -> Option<RenderObjClassId> {
        Some(RenderObjClassId::HModel)
    }
}

/// HModel instance with sub-objects
#[derive(Debug)]
pub struct HModelInstance {
    name: String,
    transform: Mat4,
    sub_objects: Vec<Box<dyn RenderObj>>,
}

impl HModelInstance {
    pub fn new(prototype: &HModelPrototype) -> Self {
        Self {
            name: prototype.name.clone(),
            transform: Mat4::IDENTITY,
            sub_objects: Vec::new(),
        }
    }
}

impl RenderObj for HModelInstance {
    fn render(&self) {
        println!("Rendering HModel: {}", self.name);
        for obj in &self.sub_objects {
            obj.render();
        }
    }

    fn get_name(&self) -> &str {
        &self.name
    }

    fn set_transform(&mut self, transform: Mat4) {
        self.transform = transform;
    }

    fn get_transform(&self) -> &Mat4 {
        &self.transform
    }
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn RenderObj> {
        Box::new(Self {
            name: self.name.clone(),
            transform: self.transform,
            sub_objects: self.sub_objects.iter().map(|obj| obj.clone_box()).collect(),
        })
    }
}

/// Material prototype containing material definitions
#[derive(Debug)]
pub struct MaterialPrototype {
    pub name: String,
    pub vertex_materials: Vec<W3dVertexMaterialStruct>,
    pub shaders: Vec<W3dShaderStruct>,
}

impl MaterialPrototype {
    pub fn new(name: String) -> Self {
        Self {
            name,
            vertex_materials: Vec::new(),
            shaders: Vec::new(),
        }
    }
}

impl Prototype for MaterialPrototype {
    fn create_instance(&self, _assets: &AssetManager) -> Option<Box<dyn RenderObj>> {
        // Materials don't create render objects directly
        None
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Texture prototype containing texture data
#[derive(Debug)]
pub struct TexturePrototype {
    pub name: String,
    pub texture_info: W3dTextureInfoStruct,
    pub texture_data: Vec<u8>,
}

impl TexturePrototype {
    pub fn new(name: String) -> Self {
        Self {
            name,
            texture_info: W3dTextureInfoStruct {
                attributes: 0,
                animation_type: 0,
                frame_count: 1,
                frame_rate: 30.0,
            },
            texture_data: Vec::new(),
        }
    }
}

impl Prototype for TexturePrototype {
    fn create_instance(&self, _assets: &AssetManager) -> Option<Box<dyn RenderObj>> {
        // Textures don't create render objects directly
        None
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Composite prototype for complex objects with multiple components
#[derive(Debug)]
pub struct CompositePrototype {
    pub name: String,
    pub sub_objects: Vec<String>, // Names of sub-prototypes
}

impl CompositePrototype {
    pub fn new(name: String) -> Self {
        Self {
            name,
            sub_objects: Vec::new(),
        }
    }

    pub fn add_sub_object(&mut self, sub_object_name: String) {
        self.sub_objects.push(sub_object_name);
    }
}

impl Prototype for CompositePrototype {
    fn create_instance(&self, assets: &AssetManager) -> Option<Box<dyn RenderObj>> {
        Some(Box::new(CompositeInstance::new(self, assets)))
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Composite instance managing multiple sub-objects
#[derive(Debug)]
pub struct CompositeInstance {
    name: String,
    transform: Mat4,
    sub_objects: Vec<Box<dyn RenderObj>>,
}

impl CompositeInstance {
    pub fn new(prototype: &CompositePrototype, assets: &AssetManager) -> Self {
        let mut instance = Self {
            name: prototype.name.clone(),
            transform: Mat4::IDENTITY,
            sub_objects: Vec::new(),
        };

        for name in &prototype.sub_objects {
            if let Some(obj) = assets.create_render_obj(name) {
                instance.sub_objects.push(obj);
            }
        }

        instance
    }

    pub fn add_sub_object(&mut self, obj: Box<dyn RenderObj>) {
        self.sub_objects.push(obj);
    }
}

impl RenderObj for CompositeInstance {
    fn render(&self) {
        println!("Rendering composite: {}", self.name);
        for obj in &self.sub_objects {
            obj.render();
        }
    }

    fn get_name(&self) -> &str {
        &self.name
    }

    fn set_transform(&mut self, transform: Mat4) {
        self.transform = transform;
        // Update all sub-objects with the relative transform
        for obj in &mut self.sub_objects {
            // Note: In a real implementation, you'd need to handle relative transforms
            obj.set_transform(transform);
        }
    }

    fn get_transform(&self) -> &Mat4 {
        &self.transform
    }
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn RenderObj> {
        Box::new(Self {
            name: self.name.clone(),
            transform: self.transform,
            sub_objects: self.sub_objects.iter().map(|obj| obj.clone_box()).collect(),
        })
    }
}

/// Box primitive prototype
#[derive(Debug)]
pub struct BoxPrototype {
    pub name: String,
    pub color: W3dRGBAStruct,
    pub center: W3dVectorStruct,
    pub extent: W3dVectorStruct,
}

impl Prototype for BoxPrototype {
    fn create_instance(&self, _assets: &AssetManager) -> Option<Box<dyn RenderObj>> {
        Some(Box::new(PrimitiveInstance {
            name: self.name.clone(),
            transform: Mat4::IDENTITY,
        }))
    }
    fn name(&self) -> &str {
        &self.name
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn class_id(&self) -> Option<RenderObjClassId> {
        Some(RenderObjClassId::AaBox)
    }
}

/// Sphere primitive prototype
#[derive(Debug)]
pub struct SpherePrototype {
    pub name: String,
    pub color: W3dRGBAStruct,
    pub center: W3dVectorStruct,
    pub radius: f32,
}

impl Prototype for SpherePrototype {
    fn create_instance(&self, _assets: &AssetManager) -> Option<Box<dyn RenderObj>> {
        Some(Box::new(PrimitiveInstance {
            name: self.name.clone(),
            transform: Mat4::IDENTITY,
        }))
    }
    fn name(&self) -> &str {
        &self.name
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn class_id(&self) -> Option<RenderObjClassId> {
        Some(RenderObjClassId::Sphere)
    }
}

/// Ring primitive prototype
#[derive(Debug)]
pub struct RingPrototype {
    pub name: String,
    pub color: W3dRGBAStruct,
    pub center: W3dVectorStruct,
    pub inner_radius: f32,
    pub outer_radius: f32,
}

impl Prototype for RingPrototype {
    fn create_instance(&self, _assets: &AssetManager) -> Option<Box<dyn RenderObj>> {
        Some(Box::new(PrimitiveInstance {
            name: self.name.clone(),
            transform: Mat4::IDENTITY,
        }))
    }
    fn name(&self) -> &str {
        &self.name
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn class_id(&self) -> Option<RenderObjClassId> {
        Some(RenderObjClassId::Ring)
    }
}

/// Null object prototype
#[derive(Debug)]
pub struct NullPrototype {
    pub name: String,
}

impl Prototype for NullPrototype {
    fn create_instance(&self, _assets: &AssetManager) -> Option<Box<dyn RenderObj>> {
        Some(Box::new(PrimitiveInstance {
            name: self.name.clone(),
            transform: Mat4::IDENTITY,
        }))
    }
    fn name(&self) -> &str {
        &self.name
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn class_id(&self) -> Option<RenderObjClassId> {
        Some(RenderObjClassId::Null)
    }
}

/// Simple renderable instance for primitives
#[derive(Debug)]
pub struct PrimitiveInstance {
    name: String,
    transform: Mat4,
}

impl RenderObj for PrimitiveInstance {
    fn render(&self) {
        println!("Rendering primitive: {}", self.name);
    }
    fn get_name(&self) -> &str {
        &self.name
    }
    fn set_transform(&mut self, transform: Mat4) {
        self.transform = transform;
    }
    fn get_transform(&self) -> &Mat4 {
        &self.transform
    }
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn RenderObj> {
        Box::new(Self {
            name: self.name.clone(),
            transform: self.transform,
        })
    }
}
