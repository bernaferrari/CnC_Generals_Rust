//! Mechanical split from `assets/models.rs`. No behavior change.
#![allow(dead_code, unused_imports)]
use super::prelude::*;
use super::w3d_anim::*;
use super::w3d_format::*;
use super::w3d_loader::*;
use super::w3d_loader_parse::*;
use super::w3d_mesh::*;
use super::w3d_mesh_build::*;
use super::*;

impl W3DModel {
    pub fn new(name: String) -> Self {
        Self {
            name,
            meshes: Vec::new(),
            materials: HashMap::new(),
            texture_names: Vec::new(),
            ww3d_mesh_models: HashMap::new(),
            bounding_box_min: Vec3::splat(f32::MAX),
            bounding_box_max: Vec3::splat(f32::MIN),
            hierarchy: None,
            hierarchies: Vec::new(),
            hlods: Vec::new(),
            hmodels: Vec::new(),
            emitters: Vec::new(),
            dazzles: Vec::new(),
            boxes: Vec::new(),
            rings: Vec::new(),
            spheres: Vec::new(),
            nulls: Vec::new(),
            collections: Vec::new(),
            aggregates: Vec::new(),
            dist_lods: Vec::new(),
            hlod_parse_failed: false,
            animations: Vec::new(),
        }
    }

    /// Retain one C++ `HTreeManager` source record without changing the
    /// legacy whole-model hierarchy selection. C++ preserves the first tree
    /// registered under an exact case-insensitive name, while existing Main
    /// rendering historically consults the most recently parsed `hierarchy`.
    /// Rigid HLOD locals now resolve by `hierarchy_name`; this convenience
    /// field remains only for mesh-only files and older fixtures.
    pub(super) fn retain_source_hierarchy(&mut self, hierarchy: W3dHierarchy) {
        let duplicate_source_name = self
            .hierarchies
            .iter()
            .any(|existing| existing.name.eq_ignore_ascii_case(&hierarchy.name));
        if !duplicate_source_name {
            self.hierarchies.push(hierarchy.clone());
        }
        self.hierarchy = Some(hierarchy);
    }

    /// Return immutable HMODEL-definition snap points in their authored W3D
    /// X/Y/Z basis.
    ///
    /// This is deliberately a source-definition query. Although
    /// `HModelDefClass::Load_W3D` retains `W3D_CHUNK_POINTS`, retail
    /// `HLodClass(HModelDefClass)` does not transfer that pointer to its own
    /// `SnapPoints` member. Consequently this API must not be used as a
    /// substitute for an active `RenderObjClass::Get_Snap_Point` call.
    /// Malformed HMODEL topology stays fail-closed, just as C++ refuses to
    /// register a prototype when its `Load_W3D` call fails.
    pub fn hmodel_source_snap_points(&self, hmodel_index: usize) -> Option<&[W3dHmodelSnapPoint]> {
        let hmodel = self.hmodels.get(hmodel_index)?;
        (!hmodel.has_invalid_records).then_some(hmodel.source_snap_points.as_slice())
    }

    /// Return one immutable source snap point, if the exact HMODEL and point
    /// index are valid. C++ indexes trusted source data directly; Main keeps
    /// this query safe rather than manufacturing an out-of-range point.
    pub fn hmodel_source_snap_point(
        &self,
        hmodel_index: usize,
        point_index: usize,
    ) -> Option<W3dHmodelSnapPoint> {
        self.hmodel_source_snap_points(hmodel_index)?
            .get(point_index)
            .copied()
    }

    /// Evaluate the constructor-selected bind-pose render topology for one
    /// exact C++ `HLodPrototypeClass` definition.
    ///
    /// `HLodLoaderClass` registers every `W3D_CHUNK_HLOD` independently and
    /// `HLodPrototypeClass::Create` passes that exact `HLodDefClass` to the
    /// constructor.  This API therefore accepts the immutable registry index
    /// instead of selecting the first HLOD or treating a source W3D file as a
    /// single aggregate.  The returned groups retain C++ render order:
    /// constructor-selected LOD children first, then `AdditionalModels`.
    ///
    /// A newly created HLOD owns its own HTree in bind pose.  As in
    /// `Animatable3DObjClass`, an empty or unavailable named HTree produces a
    /// one-pivot identity default tree; it must never borrow a different
    /// source HLOD's convenience hierarchy.  Malformed source topology,
    /// out-of-range prototype indices, invalid child identities, and invalid
    /// bone references fail closed at this isolated definition while valid
    /// sibling definitions remain usable.
    pub fn hlod_prototype_bind_pose(&self, hlod_index: usize) -> Option<W3dHlodPrototypeBindPose> {
        if self.hlod_parse_failed {
            return None;
        }

        let hlod = self.hlods.get(hlod_index)?;
        if hlod.has_invalid_trailing_records
            || hlod.name.is_empty()
            || hlod.name.as_bytes().contains(&0)
        {
            return None;
        }
        let selected_lod = hlod
            .lods
            .get(Self::cxx_constructor_selected_hlod_lod_index(hlod)?)?;

        let source_transforms = match self.source_hierarchy_for_hlod(hlod) {
            Some(hierarchy) => compute_bind_pose_global_transforms(hierarchy)?,
            // `Animatable3DObjClass::Animatable3DObjClass` calls
            // `HTreeClass::Init_Default` for an empty or unavailable named
            // hierarchy. Its single root is the render object's external
            // transform, represented by identity until the caller composes it.
            None => vec![Mat4::IDENTITY.to_cols_array()],
        };

        let pose_for_child = |child: &W3dHlodSubObject| {
            if child.name.is_empty() || child.name.as_bytes().contains(&0) {
                return None;
            }
            let bone_index = usize::try_from(child.bone_index)
                .ok()
                .filter(|index| *index < source_transforms.len())?;
            let source_transform = source_transforms.get(bone_index).copied()?;
            Some(W3dHlodAggregatePose {
                name: child.name.clone(),
                bone_index: child.bone_index,
                parent_transform: Self::w3d_transform_to_render_basis(Mat4::from_cols_array(
                    &source_transform,
                )),
                // Freshly constructed HLOD instances have no selected HAnim
                // and their bind-pose HTree pivots are visible. Root zero is
                // also forced visible by HTree.
                visible: true,
            })
        };

        let selected_lod_children = selected_lod
            .subobjects
            .iter()
            .filter_map(|child| pose_for_child(child))
            .collect();
        let additional_models = hlod
            .aggregates
            .as_ref()
            .map(|aggregates| {
                aggregates
                    .subobjects
                    .iter()
                    .filter_map(|child| pose_for_child(child))
                    .collect()
            })
            .unwrap_or_default();

        Some(W3dHlodPrototypeBindPose {
            selected_lod_children,
            additional_models,
        })
    }

    /// Return the local bind-pose palette for one independently constructed
    /// C++ HMODEL instance.
    ///
    /// `HModelPrototypeClass::Create` builds `HLodClass(HModelDef)`, whose
    /// `Animatable3DObjClass` owns the HTree named by this exact HMODEL. A
    /// missing or empty named tree becomes C++'s one-pivot identity
    /// `RootTransform`; it must never borrow another whole-file HTree or the
    /// parent Drawable animation. The external HMODEL attachment transform is
    /// intentionally not folded into this palette: `HTree::Base_Update` takes
    /// that root separately at runtime.
    pub fn hmodel_bind_pose_palette(&self, hmodel_index: usize) -> Option<Vec<Mat4>> {
        let hmodel = self.hmodels.get(hmodel_index)?;
        self.hmodel_bind_pose_source_transforms(hmodel)
            .map(|transforms| {
                transforms
                    .into_iter()
                    .map(|transform| {
                        Self::w3d_transform_to_render_basis(Mat4::from_cols_array(&transform))
                    })
                    .collect()
            })
    }

    /// Return the valid `SKIN_NODE` connections for one exact HMODEL.
    ///
    /// The connection pivot is validated against the HMODEL's own named or
    /// default HTree, but is not returned as a mesh placement matrix. C++
    /// deforms a skin with `Container->Get_HTree()` and then applies the
    /// container's outer attachment only once. Invalid individual skin nodes
    /// therefore skip without changing valid rigid sibling behavior.
    pub fn hmodel_skin_node_bindings(
        &self,
        hmodel_index: usize,
    ) -> Option<Vec<W3dHmodelSkinNodeBinding>> {
        let palette_len = self.hmodel_bind_pose_palette(hmodel_index)?.len();
        let hmodel = self.hmodels.get(hmodel_index)?;

        Some(
            hmodel
                .nodes
                .iter()
                .filter(|node| {
                    node.kind == W3dHmodelNodeKind::SkinNode
                        && !node.name.is_empty()
                        && !node.name.as_bytes().contains(&0)
                        && usize::try_from(node.bone_index)
                            .ok()
                            .is_some_and(|bone_index| bone_index < palette_len)
                })
                .map(|node| W3dHmodelSkinNodeBinding {
                    name: node.name.clone(),
                    bone_index: node.bone_index,
                })
                .collect(),
        )
    }

    /// Evaluate the rigid NODE/COLLISION_NODE records of one C++ HMODEL in
    /// its independently instantiated default HTree pose.
    ///
    /// `HModelPrototypeClass::Create` constructs `HLodClass(HModelDef)`, and
    /// `Animatable3DObjClass` clones the HTree named by the definition. If
    /// that named tree cannot be found, C++ initializes a one-pivot identity
    /// `RootTransform`; keep that exact fallback rather than borrowing a
    /// different whole-file hierarchy. SKIN_NODE records use
    /// [`Self::hmodel_bind_pose_palette`] instead because their mesh placement
    /// is the outer HMODEL attachment, not their connection bone.
    pub fn hmodel_rigid_node_poses(&self, hmodel_index: usize) -> Option<Vec<W3dHmodelNodePose>> {
        let hmodel = self.hmodels.get(hmodel_index)?;
        let source_transforms = self.hmodel_bind_pose_source_transforms(hmodel)?;

        let mut poses = Vec::new();
        for node in &hmodel.nodes {
            if !node.kind.is_currently_rigid()
                || node.name.is_empty()
                || node.name.as_bytes().contains(&0)
            {
                continue;
            }
            let Some(bone_index) = usize::try_from(node.bone_index)
                .ok()
                .filter(|index| *index < source_transforms.len())
            else {
                // C++'s trusted-data implementation would later address this
                // HTree pivot. Keep valid sibling connections independent and
                // skip only this unsafe child.
                continue;
            };
            let Some(source_transform) = source_transforms.get(bone_index).copied() else {
                continue;
            };
            poses.push(W3dHmodelNodePose {
                name: node.name.clone(),
                bone_index: node.bone_index,
                parent_transform: Self::w3d_transform_to_render_basis(Mat4::from_cols_array(
                    &source_transform,
                )),
            });
        }
        Some(poses)
    }

    /// Resolve only the HTree explicitly named by one HLOD definition.
    ///
    /// `Animatable3DObjClass` falls back to its default one-root HTree when
    /// the source name is empty or cannot be found. Returning `None` here
    /// intentionally represents that exact fallback; callers must not select
    /// the legacy convenience hierarchy merely because it happens to be from
    /// another HLOD in the same source W3D file.
    pub(super) fn source_hierarchy_for_hlod(&self, hlod: &W3dHlod) -> Option<&W3dHierarchy> {
        (!hlod.hierarchy_name.is_empty())
            .then(|| {
                self.hierarchies
                    .iter()
                    .find(|hierarchy| {
                        hierarchy
                            .name
                            .eq_ignore_ascii_case(hlod.hierarchy_name.as_str())
                    })
                    // Older hand-built fixtures may retain only the legacy
                    // field. It remains valid only when it names this exact
                    // HLOD's requested HTree.
                    .or_else(|| {
                        self.hierarchy.as_ref().filter(|hierarchy| {
                            hierarchy
                                .name
                                .eq_ignore_ascii_case(hlod.hierarchy_name.as_str())
                        })
                    })
            })
            .flatten()
    }

    /// HTree used by the live rigid HLOD / HAnim path.
    ///
    /// One source HLOD owns the tree named by `hierarchy_name`. Mesh-only
    /// files without an HLOD keep the legacy convenience field. Multiple
    /// independent HLODs stay fail-closed rather than borrowing the last
    /// parsed chunk.
    pub(super) fn hierarchy_for_sampled_hlod_or_legacy(&self) -> Option<&W3dHierarchy> {
        if self.hlod_parse_failed {
            return None;
        }
        match self.hlods.len() {
            0 => self.hierarchy.as_ref(),
            1 => {
                let hlod = self.hlods.first()?;
                if hlod.has_invalid_trailing_records {
                    return None;
                }
                self.source_hierarchy_for_hlod(hlod)
            }
            _ => None,
        }
    }

    /// HLOD `HierarchyName` values that are not present in this file's
    /// retained HTree set. C++ `Get_HTree` load-on-demand then opens
    /// `{name}.w3d`.
    pub(super) fn missing_named_hlod_hierarchy_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        for hlod in &self.hlods {
            if hlod.hierarchy_name.trim().is_empty() {
                continue;
            }
            if self.source_hierarchy_for_hlod(hlod).is_some() {
                continue;
            }
            if names
                .iter()
                .any(|existing: &String| existing.eq_ignore_ascii_case(&hlod.hierarchy_name))
            {
                continue;
            }
            names.push(hlod.hierarchy_name.clone());
        }
        names
    }

    /// Import the first case-insensitive matching HTree from a companion
    /// `{HierarchyName}.w3d` parse. First registered name wins, matching
    /// C++ `HTreeManagerClass`.
    pub(super) fn import_named_hierarchy_from(&mut self, source: &W3DModel, name: &str) {
        if name.trim().is_empty() {
            return;
        }
        if let Some(hierarchy) = source
            .hierarchies
            .iter()
            .find(|hierarchy| hierarchy.name.eq_ignore_ascii_case(name))
        {
            self.retain_source_hierarchy(hierarchy.clone());
            return;
        }
        if let Some(hierarchy) = source
            .hierarchy
            .as_ref()
            .filter(|hierarchy| hierarchy.name.eq_ignore_ascii_case(name))
        {
            self.retain_source_hierarchy(hierarchy.clone());
        }
    }

    /// Resolve only the HTree explicitly named by an HMODEL definition. The
    /// current source model can carry several HTree records; matching the
    /// convenience `hierarchy` field by position would change C++ ownership.
    pub(super) fn source_hierarchy_for_hmodel(&self, hmodel: &W3dHmodel) -> Option<&W3dHierarchy> {
        (!hmodel.hierarchy_name.is_empty())
            .then(|| {
                self.hierarchies
                    .iter()
                    .find(|hierarchy| {
                        hierarchy
                            .name
                            .eq_ignore_ascii_case(hmodel.hierarchy_name.as_str())
                    })
                    // Hand-built source fixtures from older callers may only
                    // populate the legacy field. It is usable only when it
                    // names this exact HMODEL hierarchy.
                    .or_else(|| {
                        self.hierarchy.as_ref().filter(|hierarchy| {
                            hierarchy
                                .name
                                .eq_ignore_ascii_case(hmodel.hierarchy_name.as_str())
                        })
                    })
            })
            .flatten()
    }

    /// Produce the source-space local HTree bind pose for one valid HMODEL.
    /// Keep this shared by rigid placement and skin palette construction so a
    /// named/default hierarchy decision cannot drift between the two paths.
    pub(super) fn hmodel_bind_pose_source_transforms(
        &self,
        hmodel: &W3dHmodel,
    ) -> Option<Vec<[f32; 16]>> {
        if hmodel.has_invalid_records
            || hmodel.name.is_empty()
            || hmodel.name.as_bytes().contains(&0)
        {
            return None;
        }

        match self.source_hierarchy_for_hmodel(hmodel) {
            Some(hierarchy) => compute_bind_pose_global_transforms(hierarchy),
            // `Animatable3DObjClass::Init_Default`: one visible identity root.
            None => Some(vec![Mat4::IDENTITY.to_cols_array()]),
        }
    }

    pub fn calculate_bounding_box(&mut self) {
        self.bounding_box_min = Vec3::splat(f32::MAX);
        self.bounding_box_max = Vec3::splat(f32::MIN);

        // W3D vertices are converted to the active Main render basis at import.
        // Rigid HLOD child transforms must therefore be applied exactly once here,
        // just as they are when creating a RenderItem.  Computing the transforms
        // before mutating bounds avoids borrowing `self.meshes` through both paths.
        let mesh_transforms: Vec<Option<Mat4>> = (0..self.meshes.len())
            .map(|mesh_index| self.mesh_bind_pose_local_transform(mesh_index))
            .collect();

        for (mesh, local_transform) in self.meshes.iter().zip(mesh_transforms) {
            let local_transform = local_transform.unwrap_or(mesh.transform);
            for vertex in &mesh.vertices {
                let pos = local_transform.transform_point3(Vec3::from_array(vertex.position));
                self.bounding_box_min = self.bounding_box_min.min(pos);
                self.bounding_box_max = self.bounding_box_max.max(pos);
            }
        }

        // Unsupported HLODs intentionally emit no render items.  Keep their
        // bounds finite too, so downstream culling/debug paths cannot receive
        // sentinel infinities while that source feature remains fail-closed.
        if self.bounding_box_min == Vec3::splat(f32::MAX) {
            self.bounding_box_min = Vec3::ZERO;
            self.bounding_box_max = Vec3::ZERO;
        }
    }

    /// C++ `MeshGeometryClass` sets `SKIN` only after a complete influence
    /// chunk. Either the header geometry type or a retained influence array
    /// is enough to require identity mesh placement plus an HTree palette.
    pub fn mesh_declares_skin(mesh: &W3DMesh) -> bool {
        let flagged = mesh.header.as_ref().is_some_and(|header| {
            (header.attrs & W3D_MESH_FLAG_GEOMETRY_TYPE_MASK) == W3D_MESH_FLAG_GEOMETRY_TYPE_SKIN
        });
        let influences = mesh
            .vertex_influences
            .as_ref()
            .is_some_and(|influences| !influences.is_empty());
        flagged || influences
    }

    /// C++ `MeshGeometryClass` load sets ALIGNED from CAMERA_ALIGNED (0x00010000).
    pub fn mesh_declares_camera_aligned(mesh: &W3DMesh) -> bool {
        mesh.header.as_ref().is_some_and(|header| {
            (header.attrs & W3D_MESH_FLAG_GEOMETRY_TYPE_MASK)
                == W3D_MESH_FLAG_GEOMETRY_TYPE_CAMERA_ALIGNED
        })
    }

    /// C++ `MeshGeometryClass` load sets ORIENTED from CAMERA_ORIENTED (0x00060000).
    pub fn mesh_declares_camera_oriented(mesh: &W3DMesh) -> bool {
        mesh.header.as_ref().is_some_and(|header| {
            (header.attrs & W3D_MESH_FLAG_GEOMETRY_TYPE_MASK)
                == W3D_MESH_FLAG_GEOMETRY_TYPE_CAMERA_ORIENTED
        })
    }

    /// C++ `MeshClass::Render` uses `Set_World_Identity` for SKIN
    /// (`mesh.cpp:746-771`). Deformed vertices come from
    /// `Container->Get_HTree()`; applying the HLOD bone as mesh-local as
    /// well is the double-transform shard path.
    fn skin_render_local_transform(mesh: &W3DMesh, rigid_local: Mat4) -> Mat4 {
        if Self::mesh_declares_skin(mesh) {
            Mat4::IDENTITY
        } else {
            rigid_local
        }
    }

    fn skin_local_for_mesh(&self, mesh_index: usize, rigid_local: Mat4) -> Mat4 {
        self.meshes
            .get(mesh_index)
            .map(|mesh| Self::skin_render_local_transform(mesh, rigid_local))
            .unwrap_or(rigid_local)
    }

    /// Return the render-basis local transform *and* source HTree visibility
    /// for one mesh at the requested animation frame.
    ///
    /// `None` means the mesh is not renderable through the source HLOD data:
    /// malformed HLOD, multiple independent HLODs, an unresolved selected-level
    /// child identity, an invalid bone, or an unsupported compressed visibility
    /// channel all fail closed. Aggregate children are resolved independently;
    /// their absence must not suppress valid parent geometry. Models without HLOD
    /// metadata preserve their existing local mesh transform and remain visible.
    ///
    /// An absent `animation_index` is deliberately a bind-pose request, not a
    /// request for animation zero. C++ W3DModelDraw only installs an animation
    /// explicitly selected by its current Draw state.
    pub fn mesh_local_transform_and_visibility_for_animation(
        &self,
        mesh_index: usize,
        animation_index: Option<usize>,
        animation_frame: f32,
    ) -> Option<(Mat4, bool)> {
        let binding = animation_index.map(W3dAnimationBinding::local);
        self.mesh_local_transform_and_visibility_for_binding(
            mesh_index,
            binding.as_ref(),
            animation_frame,
        )
    }

    /// As [`Self::mesh_local_transform_and_visibility_for_animation`], but
    /// retains the frozen local-or-companion HAnim selection all the way to
    /// the HLOD child. An absent binding is a bind-pose request; an invalid
    /// binding is *not* a request for local clip zero.
    pub fn mesh_local_transform_and_visibility_for_binding(
        &self,
        mesh_index: usize,
        animation_binding: Option<&W3dAnimationBinding>,
        animation_frame: f32,
    ) -> Option<(Mat4, bool)> {
        let mesh = self.meshes.get(mesh_index)?;

        if self.hlod_parse_failed {
            return None;
        }
        if self.hlods.is_empty() {
            return Some((
                Self::skin_render_local_transform(mesh, mesh.transform),
                true,
            ));
        }

        let bone_index = self.rigid_hlod_bone_index_for_mesh(mesh_index)?;
        let hierarchy = self.static_hlod_parent_context()?.2;
        let bind_pose = compute_bind_pose_global_transforms(hierarchy)?
            .get(bone_index)
            .copied()
            .map(|source_transform| (source_transform, true));
        // C++ W3DModelDraw keeps the HLod in bind pose when HAnim is missing
        // or authored for a different hierarchy. Never drop every mesh.
        let animated = animation_binding.and_then(|animation_binding| {
            if !self.animation_binding_is_compatible(animation_binding) {
                return None;
            }
            let animation = animation_binding.animation(self)?;
            let source_transform = self
                .sample_animation_binding(animation_binding, animation_frame)?
                .get(bone_index)
                .copied()?;
            let visible = animation.visibility_for_pivot(bone_index, animation_frame)?;
            Some((source_transform, visible))
        });
        let (source_transform, visible) = animated.or(bind_pose)?;

        Some((
            Self::skin_render_local_transform(
                mesh,
                Self::w3d_transform_to_render_basis(Mat4::from_cols_array(&source_transform)),
            ),
            visible,
        ))
    }

    /// Resolve each source aggregate's parent-bone pose for the frozen HAnim
    /// selection. This is the CPU-side equivalent of C++
    /// `HLodClass::Update_Sub_Object_Transforms` for `AdditionalModels`:
    /// each independently loaded child receives the exact parent HTree
    /// transform and animation-hidden state.
    ///
    /// This does not make aggregate models renderable by itself. The caller
    /// must resolve each `name` as an external render object, skip a missing
    /// asset or invalid bone individually, and compose `parent_transform`
    /// beneath the parent item world transform. Keeping that work separate
    /// prevents a source aggregate from being flattened into an unrelated
    /// parent mesh or substituted with a debug fallback.
    pub fn aggregate_attachment_poses_for_binding(
        &self,
        animation_binding: Option<&W3dAnimationBinding>,
        animation_frame: f32,
    ) -> Option<Vec<W3dHlodAggregatePose>> {
        self.aggregate_attachment_poses_for_binding_and_capture_controls(
            animation_binding,
            animation_frame,
            &[],
        )
    }

    /// As [`Self::aggregate_attachment_poses_for_binding`], but applies the
    /// ordered source-space C++ `Capture_Bone`/`Control_Bone` transforms that
    /// the current Draw module installed before `HLodClass` updates its
    /// `AdditionalModels`. This is necessary for an aggregate on a turret,
    /// recoil, or wrapper-controlled pivot to inherit that exact current pose.
    pub fn aggregate_attachment_poses_for_binding_and_capture_controls(
        &self,
        animation_binding: Option<&W3dAnimationBinding>,
        animation_frame: f32,
        capture_bone_controls: &[(i32, Mat4)],
    ) -> Option<Vec<W3dHlodAggregatePose>> {
        let (hlod, _lod, hierarchy) = self.static_hlod_parent_context()?;
        let aggregates = hlod.aggregates.as_ref()?;
        let local_transforms =
            self.local_transforms_for_animation_binding(animation_binding, animation_frame)?;
        let mut controls = vec![None; hierarchy.pivots.len()];
        let mut captured_pivots = vec![false; hierarchy.pivots.len()];
        for (raw_index, transform) in capture_bone_controls {
            let index = usize::try_from(*raw_index)
                .ok()
                .filter(|index| *index != 0 && *index < hierarchy.pivots.len())?;
            if !Self::capture_control_transform_is_affine(*transform) {
                return None;
            }
            // C++ `Control_Bone` replaces prior controls for the same pivot.
            controls[index] = Some(transform.to_cols_array());
            captured_pivots[index] = true;
        }
        let source_transforms = compute_htree_global_transforms_from_locals_with_capture_controls(
            hierarchy,
            &local_transforms,
            &controls,
        )?;
        let animation = animation_binding.and_then(|binding| binding.animation(self));
        if animation_binding.is_some() && animation.is_none() {
            return None;
        }

        let mut poses = Vec::with_capacity(aggregates.subobjects.len());
        for aggregate in &aggregates.subobjects {
            let Some(bone_index) = usize::try_from(aggregate.bone_index)
                .ok()
                .filter(|index| *index < hierarchy.pivots.len())
            else {
                // C++ `Add_Sub_Object_To_Bone` skips an aggregate with an
                // invalid bone and keeps the remaining parent HLOD intact.
                continue;
            };
            let Some(source_transform) = source_transforms.get(bone_index).copied() else {
                continue;
            };
            // HTree forces a captured pivot visible after it applies the
            // control. Its root is also always visible; neither uses a raw
            // pivot-zero visibility channel.
            let visible = if bone_index == 0 || captured_pivots[bone_index] {
                true
            } else {
                match animation {
                    Some(animation) => {
                        animation.visibility_for_pivot(bone_index, animation_frame)?
                    }
                    None => true,
                }
            };
            poses.push(W3dHlodAggregatePose {
                name: aggregate.name.clone(),
                bone_index: aggregate.bone_index,
                parent_transform: Self::w3d_transform_to_render_basis(Mat4::from_cols_array(
                    &source_transform,
                )),
                visible,
            });
        }
        Some(poses)
    }

    /// As [`Self::aggregate_attachment_poses_for_binding`], with the same
    /// primary-turret and recoil `Control_Bone` sequence used by a rigid
    /// parent mesh. C++ `HLodClass::Update_Sub_Object_Transforms` reads the
    /// parent HTree *after* `W3DModelDraw` has installed those controls, so an
    /// `AdditionalModels` child on a turret or barrel must inherit them too.
    ///
    /// The bounded visual-control implementation remains restricted to the
    /// same exact single-HLOD topology as the parent mesh helper. When that
    /// control topology is unavailable or a recoil payload is malformed, use
    /// the already-valid selected HAnim/bind pose rather than moving an
    /// aggregate through guessed names or stale indices.
    pub fn aggregate_attachment_poses_for_primary_turret_and_weapon_controls(
        &self,
        animation_binding: Option<&W3dAnimationBinding>,
        animation_frame: f32,
        primary_turret: &AuthoredDrawPrimaryTurret,
        turret_angle_degrees: f32,
        turret_pitch_degrees: f32,
        weapon_controls: &[W3dWeaponVisualControl],
    ) -> Option<Vec<W3dHlodAggregatePose>> {
        let fallback =
            self.aggregate_attachment_poses_for_binding(animation_binding, animation_frame)?;
        let Some((_hlod, hierarchy)) = self.rigid_hlod_context() else {
            return Some(fallback);
        };

        let mut capture_controls = Vec::new();
        if primary_turret.primary_fields_valid && !primary_turret.has_unsupported_alternate_turret()
        {
            if let Some((bone_index, transform)) = primary_turret
                .yaw_bone
                .as_deref()
                .and_then(|bone_name| Self::primary_turret_pivot_index(hierarchy, bone_name))
                .and_then(|bone_index| {
                    Self::primary_turret_angle_radians(
                        turret_angle_degrees,
                        primary_turret.yaw_art_angle_radians(),
                    )
                    .map(|angle| (bone_index, Mat4::from_rotation_z(angle)))
                })
            {
                capture_controls.push((i32::try_from(bone_index).ok()?, transform));
            }
            if let Some((bone_index, transform)) = primary_turret
                .pitch_bone
                .as_deref()
                .and_then(|bone_name| Self::primary_turret_pivot_index(hierarchy, bone_name))
                .and_then(|bone_index| {
                    Self::primary_turret_angle_radians(
                        turret_pitch_degrees,
                        primary_turret.pitch_art_angle_radians(),
                    )
                    .map(|angle| (bone_index, Mat4::from_rotation_y(-angle)))
                })
            {
                // `handleClientTurretPositioning` controls yaw before pitch.
                // Preserve that order, including a malformed same-pivot
                // source where pitch intentionally replaces yaw.
                capture_controls.push((i32::try_from(bone_index).ok()?, transform));
            }
        }

        for control in weapon_controls {
            let Some(pivot_index) = control
                .recoil_pivot_index
                .and_then(|index| usize::try_from(index).ok())
                .filter(|index| *index != 0 && *index < hierarchy.pivots.len())
            else {
                continue;
            };
            if !control.recoil_shift.is_finite() || control.recoil_shift < 0.0 {
                return Some(fallback);
            }
            // `handleClientRecoil` runs after turret positioning. Its later
            // slot/barrel control replaces an earlier control on the same
            // source pivot, exactly as `HTreeClass::Control_Bone` does.
            capture_controls.push((
                i32::try_from(pivot_index).ok()?,
                Mat4::from_translation(Vec3::new(-control.recoil_shift, 0.0, 0.0)),
            ));
        }

        if capture_controls.is_empty() {
            return Some(fallback);
        }
        self.aggregate_attachment_poses_for_binding_and_capture_controls(
            animation_binding,
            animation_frame,
            &capture_controls,
        )
        .or(Some(fallback))
    }

    /// As [`Self::mesh_local_transform_and_visibility_for_binding`], with an
    /// ordered C++ `HTreeClass::Capture_Bone` / `Control_Bone` control list
    /// applied in source pivot space after HAnim locals and before children
    /// inherit their parent's global transform.
    ///
    /// The controls originate in a frozen GameClient bridge submission. They
    /// are deliberately index-only: an index is valid only against this exact
    /// fresh hierarchy/HLOD, and malformed controls fail closed to the normal
    /// selected pose rather than guessing a bone by name.
    pub fn mesh_local_transform_and_visibility_for_binding_and_capture_controls(
        &self,
        mesh_index: usize,
        animation_binding: Option<&W3dAnimationBinding>,
        animation_frame: f32,
        capture_bone_controls: &[(i32, Mat4)],
    ) -> Option<(Mat4, bool)> {
        let (fallback_transform, visible) = self.mesh_local_transform_and_visibility_for_binding(
            mesh_index,
            animation_binding,
            animation_frame,
        )?;
        if capture_bone_controls.is_empty() {
            return Some((fallback_transform, visible));
        }

        let Some(mesh_bone_index) = self.rigid_hlod_bone_index_for_mesh(mesh_index) else {
            return Some((fallback_transform, visible));
        };
        let Some((_hlod, hierarchy)) = self.rigid_hlod_context() else {
            return Some((fallback_transform, visible));
        };
        let Some(local_transforms) =
            self.local_transforms_for_animation_binding(animation_binding, animation_frame)
        else {
            return Some((fallback_transform, visible));
        };

        let mut controls = vec![None; hierarchy.pivots.len()];
        for (raw_index, transform) in capture_bone_controls {
            let Some(index) = usize::try_from(*raw_index)
                .ok()
                .filter(|index| *index != 0 && *index < hierarchy.pivots.len())
            else {
                return Some((fallback_transform, visible));
            };
            if !Self::capture_control_transform_is_affine(*transform) {
                return Some((fallback_transform, visible));
            }
            // `Control_Bone` replaces its captured pivot transform. Preserve
            // the bridge order so duplicate controls retain C++ last-write
            // wins semantics.
            controls[index] = Some(transform.to_cols_array());
        }

        let Some(source_transform) =
            compute_htree_global_transforms_from_locals_with_capture_controls(
                hierarchy,
                &local_transforms,
                &controls,
            )?
            .get(mesh_bone_index)
            .copied()
        else {
            return Some((fallback_transform, visible));
        };
        Some((
            self.skin_local_for_mesh(
                mesh_index,
                Self::w3d_transform_to_render_basis(Mat4::from_cols_array(&source_transform)),
            ),
            visible,
        ))
    }

    /// Produce a render-basis skin palette after the same validated C++ HTree
    /// capture controls used by rigid HLOD children. Keeping this paired with
    /// the mesh transform prevents a controlled rigid child and skinned
    /// vertices from disagreeing in the forward pass.
    pub fn animation_palette_for_binding_and_capture_controls(
        &self,
        animation_binding: Option<&W3dAnimationBinding>,
        animation_frame: f32,
        capture_bone_controls: &[(i32, Mat4)],
    ) -> Option<Vec<Mat4>> {
        if capture_bone_controls.is_empty() {
            // Preserve the old, deliberate contract for ordinary Draw state:
            // absent animation means bind pose and does not silently upload a
            // local clip or a synthetic skin palette. C++ recoil controls are
            // different: they operate on an HTree even with no HAnim, so the
            // non-empty-control path below constructs that bind-pose palette.
            let binding = animation_binding?;
            return Some(
                self.sample_animation_binding(binding, animation_frame)?
                    .into_iter()
                    .map(|transform| {
                        Self::w3d_transform_to_render_basis(Mat4::from_cols_array(&transform))
                    })
                    .collect(),
            );
        }

        // Raw bridge controls are only meaningful against the exact rigid
        // HLOD/HTree topology that supplied their source pivot indices. A
        // hierarchy alone is insufficient: accepting a stale index against a
        // flattened or aggregate model can move unrelated geometry.
        let (_hlod, hierarchy) = self.rigid_hlod_context()?;
        let local_transforms =
            self.local_transforms_for_animation_binding(animation_binding, animation_frame)?;

        let mut controls = vec![None; hierarchy.pivots.len()];
        for (raw_index, transform) in capture_bone_controls {
            let index = usize::try_from(*raw_index)
                .ok()
                .filter(|index| *index != 0 && *index < hierarchy.pivots.len())?;
            if !Self::capture_control_transform_is_affine(*transform) {
                return None;
            }
            controls[index] = Some(transform.to_cols_array());
        }
        compute_htree_global_transforms_from_locals_with_capture_controls(
            hierarchy,
            &local_transforms,
            &controls,
        )
        .map(|transforms| {
            transforms
                .into_iter()
                .map(|transform| {
                    Self::w3d_transform_to_render_basis(Mat4::from_cols_array(&transform))
                })
                .collect()
        })
    }

    /// A bridge control is an HTree relative affine transform in source W3D
    /// pivot space. Reject projective/non-finite payloads rather than letting
    /// a malformed client submission affect the final GPU transform.
    pub(super) fn capture_control_transform_is_affine(transform: Mat4) -> bool {
        pub(super) const AFFINE_EPSILON: f32 = 1.0e-4;
        transform.is_finite()
            && transform.x_axis.w.abs() <= AFFINE_EPSILON
            && transform.y_axis.w.abs() <= AFFINE_EPSILON
            && transform.z_axis.w.abs() <= AFFINE_EPSILON
            && (transform.w_axis.w - 1.0).abs() <= AFFINE_EPSILON
    }

    /// As [`Self::mesh_local_transform_and_visibility_for_binding`], with the
    /// bounded C++ `W3DModelDraw::handleClientTurretPositioning` primary-bone
    /// control applied after the frozen source HAnim has constructed its pose.
    ///
    /// The existing HLOD transform/visibility path remains the authority for
    /// whether a mesh can render. A missing, malformed, alternate-turret, or
    /// unresolved primary binding deliberately leaves that already selected
    /// pose alone: it must never rotate the entire vehicle hull or infer a
    /// turret from a mesh name. This helper only accepts Main's active
    /// single-HLOD topology and converts the final source pose to render basis
    /// exactly once.
    pub fn mesh_local_transform_and_visibility_for_primary_turret(
        &self,
        mesh_index: usize,
        animation_binding: Option<&W3dAnimationBinding>,
        animation_frame: f32,
        primary_turret: &AuthoredDrawPrimaryTurret,
        turret_angle_degrees: f32,
        turret_pitch_degrees: f32,
    ) -> Option<(Mat4, bool)> {
        let (fallback_transform, visible) = self.mesh_local_transform_and_visibility_for_binding(
            mesh_index,
            animation_binding,
            animation_frame,
        )?;

        let Some(source_transform) = self.primary_turret_source_transform_for_mesh(
            mesh_index,
            animation_binding,
            animation_frame,
            primary_turret,
            turret_angle_degrees,
            turret_pitch_degrees,
        ) else {
            return Some((fallback_transform, visible));
        };

        Some((
            self.skin_local_for_mesh(
                mesh_index,
                Self::w3d_transform_to_render_basis(Mat4::from_cols_array(&source_transform)),
            ),
            visible,
        ))
    }

    /// As [`Self::mesh_local_transform_and_visibility_for_primary_turret`],
    /// with C++ `W3DModelDraw::handleClientRecoil` controls applied after the
    /// selected HAnim and after primary-turret capture controls.  The controls
    /// are fresh W3D pivot identities only: callers must first validate their
    /// selected source Draw state against [`Self::weapon_barrel_topology_for_authored_bindings`].
    ///
    /// A malformed/unsupported control path deliberately falls back to the
    /// already-valid turret/animation pose rather than moving an arbitrary
    /// mesh.  Muzzle visibility is kept separate from HTree capture controls
    /// because C++ hides the exact subobject on that pivot, not every sibling
    /// sharing the same bone.
    pub fn mesh_local_transform_and_visibility_for_primary_turret_and_weapon_controls(
        &self,
        mesh_index: usize,
        animation_binding: Option<&W3dAnimationBinding>,
        animation_frame: f32,
        primary_turret: &AuthoredDrawPrimaryTurret,
        turret_angle_degrees: f32,
        turret_pitch_degrees: f32,
        weapon_controls: &[W3dWeaponVisualControl],
    ) -> Option<(Mat4, bool)> {
        let (fallback_transform, visible) = self
            .mesh_local_transform_and_visibility_for_primary_turret(
                mesh_index,
                animation_binding,
                animation_frame,
                primary_turret,
                turret_angle_degrees,
                turret_pitch_degrees,
            )?;
        if weapon_controls.is_empty() {
            return Some((fallback_transform, visible));
        }

        let Some(mesh_bone_index) = self.rigid_hlod_bone_index_for_mesh(mesh_index) else {
            return Some((fallback_transform, visible));
        };
        let Some((hlod, hierarchy)) = self.rigid_hlod_context() else {
            return Some((fallback_transform, visible));
        };
        let Some(local_transforms) =
            self.local_transforms_for_animation_binding(animation_binding, animation_frame)
        else {
            return Some((fallback_transform, visible));
        };

        let mut capture_controls = vec![None; hierarchy.pivots.len()];
        if primary_turret.primary_fields_valid && !primary_turret.has_unsupported_alternate_turret()
        {
            if let Some((bone_index, transform)) = primary_turret
                .yaw_bone
                .as_deref()
                .and_then(|bone_name| Self::primary_turret_pivot_index(hierarchy, bone_name))
                .and_then(|bone_index| {
                    Self::primary_turret_angle_radians(
                        turret_angle_degrees,
                        primary_turret.yaw_art_angle_radians(),
                    )
                    .map(|angle| (bone_index, Mat4::from_rotation_z(angle).to_cols_array()))
                })
            {
                capture_controls[bone_index] = Some(transform);
            }
            if let Some((bone_index, transform)) = primary_turret
                .pitch_bone
                .as_deref()
                .and_then(|bone_name| Self::primary_turret_pivot_index(hierarchy, bone_name))
                .and_then(|bone_index| {
                    Self::primary_turret_angle_radians(
                        turret_pitch_degrees,
                        primary_turret.pitch_art_angle_radians(),
                    )
                    .map(|angle| (bone_index, Mat4::from_rotation_y(-angle).to_cols_array()))
                })
            {
                // C++ calls yaw Control_Bone before pitch. If bad source data
                // aliases them, the later pitch control replaces yaw.
                capture_controls[bone_index] = Some(transform);
            }
        }

        for control in weapon_controls {
            let Some(pivot_index) = control
                .recoil_pivot_index
                .and_then(|index| usize::try_from(index).ok())
                .filter(|index| *index != 0 && *index < hierarchy.pivots.len())
            else {
                continue;
            };
            if !control.recoil_shift.is_finite() || control.recoil_shift < 0.0 {
                return Some((fallback_transform, visible));
            }
            // `handleClientRecoil` runs after turret positioning and calls
            // Capture_Bone/Control_Bone in slot/barrel order. A later control
            // on the same pivot replaces the earlier capture transform.
            capture_controls[pivot_index] = Some(
                Mat4::from_translation(Vec3::new(-control.recoil_shift, 0.0, 0.0)).to_cols_array(),
            );
        }

        let Some(source_transform) =
            compute_htree_global_transforms_from_locals_with_capture_controls(
                hierarchy,
                &local_transforms,
                &capture_controls,
            )?
            .get(mesh_bone_index)
            .copied()
        else {
            return Some((fallback_transform, visible));
        };
        Some((
            self.skin_local_for_mesh(
                mesh_index,
                Self::w3d_transform_to_render_basis(Mat4::from_cols_array(&source_transform)),
            ),
            visible,
        ))
    }

    /// Return C++ `setMuzzleFlashHidden`'s direct visibility override for one
    /// rigid HLOD mesh.  `W3DModelDraw` changes only
    /// `Get_Sub_Object_On_Bone(0, muzzle_bone)`: the first child in the
    /// selected HLOD level on that exact pivot, not every sibling sharing the
    /// bone.  Controls are evaluated in slot/barrel order, so a malformed
    /// later alias intentionally wins just like sequential C++ calls.
    ///
    /// This is kept independent of HTree transforms.  The collector applies
    /// the override after authored Hide/Show directives, while preserving a
    /// selected HAnim's own invisible mesh state.
    pub fn muzzle_flash_visibility_override_for_mesh(
        &self,
        mesh_index: usize,
        weapon_controls: &[W3dWeaponVisualControl],
    ) -> Option<bool> {
        let (mesh_subobject, mesh_bone_index) = self.rigid_hlod_subobject_for_mesh(mesh_index)?;
        let (hlod, _hierarchy) = self.rigid_hlod_context()?;
        let mesh_bone_index = u32::try_from(mesh_bone_index).ok()?;
        let first_child_for_pivot = |pivot_index: u32| {
            hlod.lods.first().and_then(|lod| {
                lod.subobjects
                    .iter()
                    .find(|child| child.bone_index == pivot_index)
            })
        };

        let mut override_visibility = None;
        for control in weapon_controls {
            let Some(pivot_index) = control.muzzle_flash_pivot_index else {
                continue;
            };
            if mesh_bone_index != pivot_index {
                continue;
            }
            if first_child_for_pivot(pivot_index).is_some_and(|child| {
                child
                    .name
                    .eq_ignore_ascii_case(mesh_subobject.name.as_str())
            }) {
                override_visibility = Some(control.muzzle_flash_visible);
            }
        }
        override_visibility
    }

    /// Return the source-space HTree transform for one rigid HLOD mesh after
    /// C++-ordered primary turret capture controls. `None` means no safe
    /// primary control exists, not that the mesh itself is unavailable; the
    /// caller uses its already validated selected-animation/bind-pose value.
    pub(super) fn primary_turret_source_transform_for_mesh(
        &self,
        mesh_index: usize,
        animation_binding: Option<&W3dAnimationBinding>,
        animation_frame: f32,
        primary_turret: &AuthoredDrawPrimaryTurret,
        turret_angle_degrees: f32,
        turret_pitch_degrees: f32,
    ) -> Option<[f32; 16]> {
        if !primary_turret.primary_fields_valid
            || primary_turret.has_unsupported_alternate_turret()
            || !primary_turret.has_primary_bone()
        {
            return None;
        }

        // This is intentionally the same one-HLOD/one-LOD/compatible-HTree
        // gate as the normal rigid-child transform path. A bare mesh or a
        // flattened multi-LOD name cannot become an implicit turret target.
        let mesh_bone_index = self.rigid_hlod_bone_index_for_mesh(mesh_index)?;
        let (_hlod, hierarchy) = self.rigid_hlod_context()?;

        let yaw_control = primary_turret
            .yaw_bone
            .as_deref()
            .and_then(|bone_name| Self::primary_turret_pivot_index(hierarchy, bone_name))
            .and_then(|bone_index| {
                Self::primary_turret_angle_radians(
                    turret_angle_degrees,
                    primary_turret.yaw_art_angle_radians(),
                )
                .map(|angle| (bone_index, Mat4::from_rotation_z(angle).to_cols_array()))
            });
        let pitch_control = primary_turret
            .pitch_bone
            .as_deref()
            .and_then(|bone_name| Self::primary_turret_pivot_index(hierarchy, bone_name))
            .and_then(|bone_index| {
                Self::primary_turret_angle_radians(
                    turret_pitch_degrees,
                    primary_turret.pitch_art_angle_radians(),
                )
                // C++ uses Rotate_Y(-turretPitch) after adding its authored
                // `TurretArtPitch` offset.
                .map(|angle| (bone_index, Mat4::from_rotation_y(-angle).to_cols_array()))
            });

        if yaw_control.is_none() && pitch_control.is_none() {
            return None;
        }

        let local_transforms =
            self.local_transforms_for_animation_binding(animation_binding, animation_frame)?;
        let mut capture_controls = vec![None; hierarchy.pivots.len()];
        if let Some((bone_index, transform)) = yaw_control {
            capture_controls[bone_index] = Some(transform);
        }
        if let Some((bone_index, transform)) = pitch_control {
            // `handleClientTurretPositioning` calls yaw then pitch. If an
            // invalid source uses one exact bone for both, the latter
            // Control_Bone call replaces the former capture transform.
            capture_controls[bone_index] = Some(transform);
        }

        compute_htree_global_transforms_from_locals_with_capture_controls(
            hierarchy,
            &local_transforms,
            &capture_controls,
        )?
        .get(mesh_bone_index)
        .copied()
    }

    /// Resolve a C++ `NameKey` only against an exact HTree pivot. Pivot zero
    /// is C++'s "unresolved/no bone" sentinel in `validateTurretInfo`, so a
    /// root-name match may not turn into a whole-model rotation.
    pub(super) fn primary_turret_pivot_index(
        hierarchy: &W3dHierarchy,
        bone_name: &str,
    ) -> Option<usize> {
        hierarchy
            .pivots
            .iter()
            .position(|pivot| pivot.name.eq_ignore_ascii_case(bone_name))
            .filter(|bone_index| *bone_index != 0)
    }

    pub(super) fn primary_turret_angle_radians(
        gameplay_degrees: f32,
        art_radians: f32,
    ) -> Option<f32> {
        let angle = gameplay_degrees.to_radians() + art_radians;
        (gameplay_degrees.is_finite() && art_radians.is_finite() && angle.is_finite())
            .then_some(angle)
    }

    /// Rebuild C++ `ModelConditionInfo::m_weaponBarrelInfoVec` for one frozen
    /// selected Draw state using only its exact authored bases and this exact
    /// active W3D hierarchy.
    ///
    /// The bounded Main renderer supports only the same rigid single-HLOD
    /// topology used by its transform path. Missing/malformed Draw data,
    /// malformed HLODs, multi-LOD/aggregate content, or an incompatible
    /// hierarchy return `None`; callers must leave recoil idle instead of
    /// inferring a barrel from a mesh/model name. A valid source state with no
    /// retained bones returns `Some` with empty vectors, matching C++'s lack
    /// of `WeaponBarrelInfo` for that slot.
    pub fn weapon_barrel_topology_for_authored_bindings(
        &self,
        bindings: &AuthoredDrawWeaponBoneBindings,
    ) -> Option<W3dWeaponBarrelTopology> {
        if !bindings.source_fields_valid || self.hlod_parse_failed {
            return None;
        }
        let (_hlod, hierarchy) = self.rigid_hlod_context()?;
        Some(W3dWeaponBarrelTopology {
            slots: std::array::from_fn(|slot| {
                Self::weapon_barrels_for_authored_slot(hierarchy, &bindings.slots[slot])
            }),
        })
    }

    /// Mirror C++ `validateWeaponBarrelInfo`: scan every supplied base with
    /// `%02d` indices 01 through 99, stop at the first all-missing record,
    /// and use unadorned names only when no numbered record was found. A
    /// numbered muzzle flash may reuse the previous numbered FX pivot exactly
    /// like the retail multi-flash exception in C++.
    pub(super) fn weapon_barrels_for_authored_slot(
        hierarchy: &W3dHierarchy,
        authored: &AuthoredDrawWeaponBoneSlot,
    ) -> Vec<W3dWeaponBarrelBinding> {
        let has_any_base = authored.fire_fx_bone_base.is_some()
            || authored.recoil_bone_base.is_some()
            || authored.muzzle_flash_bone_base.is_some()
            || authored.launch_bone_base.is_some();
        if !has_any_base {
            return Vec::new();
        }

        let mut numbered = Vec::new();
        let mut previous_fire_fx = None;
        for index in 1..=99u8 {
            let mut binding = W3dWeaponBarrelBinding {
                fire_fx_pivot_index: authored.fire_fx_bone_base.as_deref().and_then(|base| {
                    Self::pristine_pivot_index(hierarchy, &format!("{base}{index:02}"))
                }),
                recoil_pivot_index: authored.recoil_bone_base.as_deref().and_then(|base| {
                    Self::pristine_pivot_index(hierarchy, &format!("{base}{index:02}"))
                }),
                muzzle_flash_pivot_index: authored.muzzle_flash_bone_base.as_deref().and_then(
                    |base| Self::pristine_pivot_index(hierarchy, &format!("{base}{index:02}")),
                ),
                launch_pivot_index: authored.launch_bone_base.as_deref().and_then(|base| {
                    Self::pristine_pivot_index(hierarchy, &format!("{base}{index:02}"))
                }),
            };
            if binding.fire_fx_pivot_index.is_none() && binding.muzzle_flash_pivot_index.is_some() {
                binding.fire_fx_pivot_index = previous_fire_fx;
            }
            if !binding.has_any_binding() {
                break;
            }
            previous_fire_fx = binding.fire_fx_pivot_index;
            numbered.push(binding);
        }

        if !numbered.is_empty() {
            return numbered;
        }

        let unadorned = W3dWeaponBarrelBinding {
            fire_fx_pivot_index: authored
                .fire_fx_bone_base
                .as_deref()
                .and_then(|base| Self::pristine_pivot_index(hierarchy, base)),
            recoil_pivot_index: authored
                .recoil_bone_base
                .as_deref()
                .and_then(|base| Self::pristine_pivot_index(hierarchy, base)),
            muzzle_flash_pivot_index: authored
                .muzzle_flash_bone_base
                .as_deref()
                .and_then(|base| Self::pristine_pivot_index(hierarchy, base)),
            launch_pivot_index: authored
                .launch_bone_base
                .as_deref()
                .and_then(|base| Self::pristine_pivot_index(hierarchy, base)),
        };
        unadorned
            .has_any_binding()
            .then_some(unadorned)
            .into_iter()
            .collect()
    }

    /// C++ `findPristineBone` treats index zero as an unresolved/no-bone
    /// sentinel in the weapon and turret paths. Never let a matching root
    /// pivot become a whole-model recoil/launch binding.
    pub(super) fn pristine_pivot_index(hierarchy: &W3dHierarchy, name: &str) -> Option<u32> {
        hierarchy
            .pivots
            .iter()
            .position(|pivot| pivot.name.eq_ignore_ascii_case(name))
            .filter(|index| *index != 0)
            .and_then(|index| u32::try_from(index).ok())
    }

    /// Build source-space local pivot matrices from either an explicitly
    /// frozen compatible HAnim or the HTree bind pose. Absent binding is
    /// deliberately bind pose; it must not select local animation zero.
    pub(super) fn local_transforms_for_animation_binding(
        &self,
        animation_binding: Option<&W3dAnimationBinding>,
        animation_frame: f32,
    ) -> Option<Vec<[f32; 16]>> {
        let hierarchy = self.hierarchy_for_sampled_hlod_or_legacy()?;
        let animation = match animation_binding {
            Some(binding) => {
                if !self.animation_binding_is_compatible(binding) {
                    return None;
                }
                binding.animation(self)?
            }
            None => return Some(hierarchy.pivots.iter().map(mat4_from_pivot).collect()),
        };
        sample_animation_local_transforms(hierarchy, animation, animation_frame)
    }

    /// Apply the frozen active `W3DModelDraw` `ShowSubObject`/`HideSubObject`
    /// state to one already-resolved rigid HLOD mesh.
    ///
    /// C++ first looks up a full subobject name, then the exact substring after
    /// its first dot, and applies the directive to that HTree bone plus all of
    /// its descendants.  Main keeps that lookup strictly inside one supported
    /// source HLOD's retained child records; it never guesses from an arbitrary
    /// mesh, template, or suffix.  Missing/unsupported metadata consequently
    /// leaves the mesh unchanged here (the transform path remains separately
    /// fail-closed for unsupported HLODs).
    pub fn mesh_visible_for_authored_subobject_directives(
        &self,
        mesh_index: usize,
        directives: &[AuthoredDrawSubobjectVisibility],
    ) -> bool {
        if directives.is_empty() || self.hlod_parse_failed || self.hlods.is_empty() {
            return true;
        }

        let Some((mesh_subobject, mesh_bone_index)) =
            self.rigid_hlod_subobject_for_mesh(mesh_index)
        else {
            return true;
        };
        let Some((hlod, lod, hierarchy)) = self.rigid_hlod_static_lod_context() else {
            return true;
        };

        // `ModelConditionInfo::m_hideShowVec` is iterated in its retained
        // declaration order.  A later directive affecting the same child or
        // an ancestor intentionally wins.
        let mut visible = true;
        for directive in directives {
            let Some(target_subobject) =
                Self::rigid_hlod_subobject_for_authored_directive(hlod, lod, &directive.name)
            else {
                continue;
            };
            let Some(target_bone_index) = usize::try_from(target_subobject.bone_index)
                .ok()
                .filter(|bone_index| *bone_index < hierarchy.pivots.len())
            else {
                continue;
            };
            // C++ hides the exact looked-up RenderObj directly, then visits
            // *strict* HTree descendants. A separate sibling on the same bone
            // is neither the named child nor a descendant and must stay intact.
            if mesh_subobject
                .name
                .eq_ignore_ascii_case(target_subobject.name.as_str())
                || Self::hierarchy_bone_is_strict_descendant(
                    hierarchy,
                    mesh_bone_index,
                    target_bone_index,
                )
            {
                visible = !directive.hidden;
            }
        }
        visible
    }

    /// Backwards-compatible transform-only facade for callers that have not
    /// yet retained a source Draw-state animation identity.  An out-of-range
    /// legacy index preserves the old bind-pose fallback rather than silently
    /// selecting another W3D animation.
    pub fn mesh_local_transform_for_animation(
        &self,
        mesh_index: usize,
        animation_index: usize,
        animation_frame: f32,
    ) -> Option<Mat4> {
        self.mesh_local_transform_and_visibility_for_animation(
            mesh_index,
            (animation_index < self.animations.len()).then_some(animation_index),
            animation_frame,
        )
        .map(|(transform, _visible)| transform)
    }

    /// Return the rigid mesh transform in its HTree bind pose.  This is used for
    /// model culling/bounds and has the same fail-closed identity checks as the
    /// animated render path above.
    pub(super) fn mesh_bind_pose_local_transform(&self, mesh_index: usize) -> Option<Mat4> {
        let mesh = self.meshes.get(mesh_index)?;

        if self.hlod_parse_failed {
            return None;
        }
        if self.hlods.is_empty() {
            return Some(Self::skin_render_local_transform(mesh, mesh.transform));
        }

        let bone_index = self.rigid_hlod_bone_index_for_mesh(mesh_index)?;
        let hierarchy = self.static_hlod_parent_context()?.2;
        let source_transform = compute_bind_pose_global_transforms(hierarchy)?
            .get(bone_index)
            .copied()?;
        Some(Self::skin_render_local_transform(
            mesh,
            Self::w3d_transform_to_render_basis(Mat4::from_cols_array(&source_transform)),
        ))
    }

    /// Resolve a flattened Main mesh back to the precise source HLOD record.
    ///
    /// C++ asks its asset manager for the exact `HLOD.Name.MeshName` render object
    /// and then assigns that object the source `BoneIndex`.  In Main, meshes reside
    /// in the same loaded W3D stream, so require the mesh header's own
    /// `ContainerName` plus that exact composed source identity.  Do not use a
    /// suffix, pivot-name, or template-name fallback here.
    pub(super) fn rigid_hlod_bone_index_for_mesh(&self, mesh_index: usize) -> Option<usize> {
        self.rigid_hlod_subobject_for_mesh(mesh_index)
            .map(|(_subobject, bone_index)| bone_index)
    }

    /// Resolve one flattened Main mesh to its exact retained rigid HLOD child
    /// and source HTree bone.  Keeping the child identity is necessary for
    /// `ShowSubObject`/`HideSubObject`: C++ directly changes only the matched
    /// render object before it recursively changes descendant bones.
    pub(super) fn rigid_hlod_subobject_for_mesh(
        &self,
        mesh_index: usize,
    ) -> Option<(&W3dHlodSubObject, usize)> {
        let (hlod, lod, hierarchy) = self.rigid_hlod_static_lod_context()?;

        let mesh = self.meshes.get(mesh_index)?;
        if mesh.container_name.is_empty()
            || !mesh.container_name.eq_ignore_ascii_case(hlod.name.as_str())
        {
            return None;
        }
        let source_identity = format!("{}.{}", mesh.container_name, mesh.name);
        let subobject = lod.subobjects.iter().find(|subobject| {
            subobject
                .name
                .eq_ignore_ascii_case(source_identity.as_str())
        })?;

        let bone_index = usize::try_from(subobject.bone_index).ok()?;
        (bone_index < hierarchy.pivots.len()).then_some((subobject, bone_index))
    }

    /// C++ `HLodClass` construction starts from `CurLod == 0`, calls
    /// `Calculate_Cost_Value_Arrays(1.0f, ...)`, then raises it to the returned
    /// minimum level.  It uses a strict `<` comparison against each ordered
    /// `MaxScreenSize`; if every level is below one pixel, the final (highest
    /// detail) level is selected.  The W3D exporter stores levels low-to-high.
    ///
    /// Generals' `RTS3DScene` explicitly disables its later dynamic
    /// `Prepare_LOD`/optimizer calls, so this frozen construction selection is
    /// the only HLOD selection Main may perform without inventing behavior.
    /// Malformed thresholds fail closed instead of making an arbitrary level
    /// visible.
    pub(super) fn cxx_constructor_selected_hlod_lod_index(hlod: &W3dHlod) -> Option<usize> {
        let lods = &hlod.lods;
        if lods.is_empty() || lods.iter().any(|lod| !lod.max_screen_size.is_finite()) {
            return None;
        }

        let mut min_lod = 0;
        while min_lod < lods.len() && lods[min_lod].max_screen_size < 1.0 {
            min_lod += 1;
        }
        Some(min_lod.min(lods.len() - 1))
    }

    /// The source-valid parent HLOD/HTree topology shared by normal rigid
    /// children and C++ `AdditionalModels`. Aggregate entries are deliberately
    /// allowed here so their parent-bone poses can be prepared without
    /// pretending their external geometry is already rendered.
    ///
    /// C++ `Animatable3DObjClass` clones `Get_HTree(hierarchy_name)`, never
    /// the last hierarchy chunk parsed from the same file. Resolve through
    /// [`Self::source_hierarchy_for_hlod`] so a later unrelated tree cannot
    /// hard-fail every mesh to identity.
    pub(super) fn static_hlod_parent_context(
        &self,
    ) -> Option<(&W3dHlod, &W3dHlodLod, &W3dHierarchy)> {
        if self.hlod_parse_failed || self.hlods.len() != 1 {
            return None;
        }
        let hlod = self.hlods.first()?;
        if hlod.has_invalid_trailing_records || hlod.name.is_empty() {
            return None;
        }
        let lod = hlod
            .lods
            .get(Self::cxx_constructor_selected_hlod_lod_index(hlod)?)?;

        let hierarchy = self.source_hierarchy_for_hlod(hlod)?;
        Some((hlod, lod, hierarchy))
    }

    /// The bounded rigid geometry topology that can safely use the C++
    /// constructor-selected static level: one source HLOD, an exact matching
    /// source hierarchy, and an intact selected level.
    ///
    /// `HLodClass` creates every aggregate independently. A missing aggregate
    /// render object or an aggregate with an invalid parent bone does not
    /// invalidate the selected parent LOD, so retained aggregate metadata must
    /// never hide otherwise-valid parent geometry. Proxies likewise remain
    /// non-rendering application data.
    /// Every caller must preserve this gate rather than treating flattened
    /// mesh names as a substitute.
    pub(super) fn rigid_hlod_static_lod_context(
        &self,
    ) -> Option<(&W3dHlod, &W3dHlodLod, &W3dHierarchy)> {
        self.static_hlod_parent_context()
    }

    /// The deliberately narrower topology required by current turret/recoil
    /// state.  Static multi-LOD geometry is safe above, but visual controls
    /// still require a separately validated one-level HLOD rather than being
    /// projected onto a selected level by inference.
    pub(super) fn rigid_hlod_context(&self) -> Option<(&W3dHlod, &W3dHierarchy)> {
        let (hlod, _lod, hierarchy) = self.rigid_hlod_static_lod_context()?;
        (hlod.lods.len() == 1).then_some((hlod, hierarchy))
    }

    /// Resolve C++ `RenderObjClass::Get_Sub_Object_By_Name` only through a
    /// structurally valid retained HLOD child.  Its first pass compares the
    /// full source child name; its second pass compares the exact text after
    /// the first dot.  We require the child record to have this HLOD's exact
    /// prefix, so no unrelated mesh-name suffix can become visibility authority.
    pub(super) fn rigid_hlod_subobject_for_authored_directive<'a>(
        hlod: &'a W3dHlod,
        lod: &'a W3dHlodLod,
        directive_name: &str,
    ) -> Option<&'a W3dHlodSubObject> {
        let directive_name = directive_name.trim();
        if directive_name.is_empty() {
            return None;
        }
        let subobjects = &lod.subobjects;
        subobjects
            .iter()
            .find(|subobject| {
                Self::rigid_hlod_child_leaf_name(hlod, subobject).is_some()
                    && subobject.name.eq_ignore_ascii_case(directive_name)
            })
            .or_else(|| {
                subobjects.iter().find(|subobject| {
                    Self::rigid_hlod_child_leaf_name(hlod, subobject)
                        .is_some_and(|leaf_name| leaf_name.eq_ignore_ascii_case(directive_name))
                })
            })
    }

    /// Return the C++ first-dot suffix only for a source record structurally
    /// owned by this exact HLOD.  A bare or differently-prefixed name is not a
    /// valid child mapping in Main's bounded rigid HLOD implementation.
    pub(super) fn rigid_hlod_child_leaf_name<'a>(
        hlod: &W3dHlod,
        subobject: &'a W3dHlodSubObject,
    ) -> Option<&'a str> {
        let (prefix, leaf_name) = subobject.name.split_once('.')?;
        (!leaf_name.is_empty() && prefix.eq_ignore_ascii_case(hlod.name.as_str()))
            .then_some(leaf_name)
    }

    /// Whether `bone_index` lies strictly below `ancestor_bone_index` in the
    /// source HTree. The exact direct target is handled separately, matching
    /// C++ `doHideShowSubObjs` plus `doHideShowBoneSubObjs`. The bounded walk
    /// rejects malformed roots/cycles instead of treating invalid parent data
    /// as visible geometry.
    pub(super) fn hierarchy_bone_is_strict_descendant(
        hierarchy: &W3dHierarchy,
        bone_index: usize,
        ancestor_bone_index: usize,
    ) -> bool {
        let mut current_bone_index = bone_index;
        for _ in 0..hierarchy.pivots.len() {
            let Some(pivot) = hierarchy.pivots.get(current_bone_index) else {
                return false;
            };
            if pivot.parent_idx == u32::MAX {
                return false;
            }
            let Ok(parent_bone_index) = usize::try_from(pivot.parent_idx) else {
                return false;
            };
            if parent_bone_index >= hierarchy.pivots.len()
                || parent_bone_index == current_bone_index
            {
                return false;
            }
            if parent_bone_index == ancestor_bone_index {
                return true;
            }
            current_bone_index = parent_bone_index;
        }
        false
    }

    /// Convert a source W3D Z-up matrix to the render basis used by imported
    /// `W3DVertex` payloads.  The axis swap is its own inverse.
    pub(super) fn w3d_transform_to_render_basis(transform: Mat4) -> Mat4 {
        let axis = Mat4::from_cols_array(&[
            1.0, 0.0, 0.0, 0.0, // X stays X
            0.0, 0.0, 1.0, 0.0, // source Y becomes render Z
            0.0, 1.0, 0.0, 0.0, // source Z becomes render Y
            0.0, 0.0, 0.0, 1.0,
        ]);
        axis * transform * axis
    }

    /// Get the list of animation names available on this model.
    pub fn animation_names(&self) -> Vec<&str> {
        self.animations.iter().map(|a| a.name.as_str()).collect()
    }

    /// Find an animation index by name (case-insensitive).
    pub fn find_animation_index(&self, name: &str) -> Option<usize> {
        let lower = name.to_ascii_lowercase();
        self.animations
            .iter()
            .position(|a| a.name.to_ascii_lowercase() == lower)
    }

    /// Resolve a C++ `W3DModelDraw` animation identity against this exact W3D
    /// file.  Retail Object INIs commonly use the canonical
    /// `Hierarchy.Animation` spelling while a raw W3D animation header stores
    /// those two source records separately.  This is an exact qualified-record
    /// comparison, not a basename/suffix heuristic.
    pub fn find_animation_index_for_draw_identity(&self, identity: &str) -> Option<usize> {
        self.animations
            .iter()
            .position(|animation| animation.matches_draw_identity(identity))
    }

    /// Resolve an exact Draw identity only when this geometry file itself
    /// carries a compatible raw HAnim. The caller may then try C++'s companion
    /// `Animation.w3d` rule; it must never substitute a local clip by ordinal.
    pub fn local_animation_binding_for_draw_identity(
        &self,
        identity: &str,
    ) -> Option<W3dAnimationBinding> {
        let binding =
            W3dAnimationBinding::local(self.find_animation_index_for_draw_identity(identity)?);
        self.animation_binding_is_compatible(&binding)
            .then_some(binding)
    }

    /// Return whether a frozen Draw HAnim can be sampled against this model's
    /// actual hierarchy. Companion clips remain separate assets, but C++ binds
    /// them to the named HTree only; a matching clip name alone is insufficient.
    pub fn animation_binding_is_compatible(&self, binding: &W3dAnimationBinding) -> bool {
        let Some(hierarchy) = self.hierarchy_for_sampled_hlod_or_legacy() else {
            return false;
        };
        let Some(animation) = binding.animation(self) else {
            return false;
        };
        if animation.hierarchy_name.trim().is_empty()
            || animation.name.trim().is_empty()
            || animation.num_frames == 0
            || animation.frame_rate == 0
            || !animation
                .hierarchy_name
                .eq_ignore_ascii_case(hierarchy.name.as_str())
        {
            return false;
        }

        match binding {
            W3dAnimationBinding::Local { .. } => true,
            W3dAnimationBinding::Companion { identity, .. } => {
                !animation.source_is_compressed && animation.matches_draw_identity(identity)
            }
        }
    }

    /// Get animation metadata: (num_frames, frame_rate) for the given animation.
    pub fn animation_metadata(&self, anim_index: usize) -> Option<(u32, u32)> {
        let anim = self.animations.get(anim_index)?;
        Some((anim.num_frames, anim.frame_rate))
    }

    /// Metadata for one exact frozen animation binding. An incompatible
    /// companion is treated as unavailable so callers stay in bind pose.
    pub fn animation_binding_metadata(&self, binding: &W3dAnimationBinding) -> Option<(u32, u32)> {
        self.animation_binding_is_compatible(binding)
            .then(|| binding.animation(self))
            .flatten()
            .map(|animation| (animation.num_frames, animation.frame_rate))
    }

    /// Sample an animation at the given frame, producing per-bone global transforms.
    ///
    /// Returns a Vec of column-major 4x4 matrices indexed by pivot (bone) index,
    /// or `None` if the animation or hierarchy is missing.
    ///
    /// The frame parameter is a continuous value; fractional parts interpolate
    /// between adjacent keyframes.
    pub fn sample_animation(&self, anim_index: usize, frame: f32) -> Option<Vec<[f32; 16]>> {
        let anim = self.animations.get(anim_index)?;
        self.sample_animation_data(anim, frame)
    }

    /// Sample a selected local or exact companion HAnim. This performs the
    /// hierarchy validation at the final palette boundary too, so an invalid
    /// companion cannot turn into a local animation-zero pose downstream.
    pub fn sample_animation_binding(
        &self,
        binding: &W3dAnimationBinding,
        frame: f32,
    ) -> Option<Vec<[f32; 16]>> {
        if !self.animation_binding_is_compatible(binding) {
            return None;
        }
        self.sample_animation_data(binding.animation(self)?, frame)
    }

    pub(super) fn sample_animation_data(
        &self,
        anim: &W3dAnimation,
        frame: f32,
    ) -> Option<Vec<[f32; 16]>> {
        let hierarchy = self.hierarchy_for_sampled_hlod_or_legacy()?;
        let local_transforms = sample_animation_local_transforms(hierarchy, anim, frame)?;
        compute_htree_global_transforms_from_locals(hierarchy, &local_transforms)
    }
}

#[cfg(test)]
mod hlod_named_htree_tests {
    use super::super::prelude::{Mat4, Vec3, W3dVertInfStruct};
    use super::super::{
        W3DMesh, W3DModel, W3dHierarchy, W3dHlod, W3dHlodLod, W3dHlodSubObject, W3dPivot,
    };
    fn test_pivot(name: &str, parent_idx: u32, translation: [f32; 3]) -> W3dPivot {
        W3dPivot {
            name: name.to_string(),
            parent_idx,
            translation,
            euler_angles: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
        }
    }

    fn named_hlod_model(hierarchy: W3dHierarchy, bone_index: u32) -> W3DModel {
        let hierarchy_name = hierarchy.name.clone();
        let mut model = W3DModel::new("named_hlod".to_string());
        model.retain_source_hierarchy(hierarchy);
        model.hlods.push(W3dHlod {
            version: 0x0001_0000,
            name: "HLODROOT".to_string(),
            hierarchy_name,
            lods: vec![W3dHlodLod {
                max_screen_size: f32::MAX,
                subobjects: vec![W3dHlodSubObject {
                    name: "HLODROOT.RIGID".to_string(),
                    bone_index,
                }],
            }],
            aggregates: None,
            proxies: None,
            has_unrendered_aggregates: false,
            has_invalid_trailing_records: false,
        });
        let mut mesh = W3DMesh::new("RIGID".to_string());
        mesh.container_name = "HLODROOT".to_string();
        mesh.transform = Mat4::from_translation(Vec3::new(7.0, 8.0, 9.0));
        model.meshes.push(mesh);
        model
    }

    #[test]
    fn failed_hlod_bind_skips_instead_of_identity_draw() {
        // C++ `HLodClass::Update_Sub_Object_Transforms` (`hlod.cpp:3236-3245`)
        // only walks created LOD children. A failed bone bind is not
        // `Set_Transform(IDENTITY)`.
        let hierarchy = W3dHierarchy {
            name: "RIG_HIER".to_string(),
            pivots: vec![
                test_pivot("ROOT", u32::MAX, [0.0; 3]),
                test_pivot("BONE", 0, [10.0, 20.0, 30.0]),
            ],
            pivot_fixups: Vec::new(),
        };
        let mut model = named_hlod_model(hierarchy, 1);
        model.meshes[0].container_name = "OTHER".to_string();
        assert!(
            model
                .mesh_local_transform_and_visibility_for_binding(0, None, 0.0)
                .is_none(),
            "an unresolved HLOD child must skip, not draw at mesh.transform/identity"
        );

        let mut missing_tree = named_hlod_model(
            W3dHierarchy {
                name: "OTHER_TREE".to_string(),
                pivots: vec![
                    test_pivot("ROOT", u32::MAX, [0.0; 3]),
                    test_pivot("BONE", 0, [99.0, 88.0, 77.0]),
                ],
                pivot_fixups: Vec::new(),
            },
            1,
        );
        missing_tree.hlods[0].hierarchy_name = "MISSING_TREE".to_string();
        assert!(
            missing_tree
                .mesh_local_transform_and_visibility_for_binding(0, None, 0.0)
                .is_none(),
            "a missing named HTree must not identity-draw HLOD children"
        );
    }

    #[test]
    fn rigid_hlod_uses_named_htree_not_last_parsed_unrelated_tree() {
        // C++ `Animatable3DObjClass` clones `Get_HTree(hierarchy_name)`.
        let named = W3dHierarchy {
            name: "RIG_HIER".to_string(),
            pivots: vec![
                test_pivot("ROOT", u32::MAX, [0.0; 3]),
                test_pivot("BONE", 0, [10.0, 20.0, 30.0]),
            ],
            pivot_fixups: Vec::new(),
        };
        let mut model = named_hlod_model(named, 1);
        model.hierarchy = Some(W3dHierarchy {
            name: "UNRELATED".to_string(),
            pivots: vec![
                test_pivot("ROOT", u32::MAX, [0.0; 3]),
                test_pivot("BONE", 0, [99.0, 88.0, 77.0]),
            ],
            pivot_fixups: Vec::new(),
        });

        let (transform, visible) = model
            .mesh_local_transform_and_visibility_for_binding(0, None, 0.0)
            .expect("named HTree must resolve the authored child");
        assert!(visible);
        let pos = transform.w_axis.truncate();
        assert!(
            (pos - Vec3::new(10.0, 30.0, 20.0)).length() < 0.0001,
            "named HTree translation must win over the last-parsed tree, got {pos:?}"
        );
    }

    #[test]
    fn companion_named_htree_import_enables_failed_bind_to_resolve() {
        let mut model = W3DModel::new("geometry".to_string());
        model.hlods.push(W3dHlod {
            version: 0x0001_0000,
            name: "HLODROOT".to_string(),
            hierarchy_name: "COMPANION".to_string(),
            lods: vec![W3dHlodLod {
                max_screen_size: f32::MAX,
                subobjects: vec![W3dHlodSubObject {
                    name: "HLODROOT.RIGID".to_string(),
                    bone_index: 1,
                }],
            }],
            aggregates: None,
            proxies: None,
            has_unrendered_aggregates: false,
            has_invalid_trailing_records: false,
        });
        let mut mesh = W3DMesh::new("RIGID".to_string());
        mesh.container_name = "HLODROOT".to_string();
        model.meshes.push(mesh);

        assert_eq!(
            model.missing_named_hlod_hierarchy_names(),
            vec!["COMPANION".to_string()]
        );
        assert!(
            model
                .mesh_local_transform_and_visibility_for_binding(0, None, 0.0)
                .is_none(),
            "before companion load the named HTree miss must skip"
        );

        let mut companion = W3DModel::new("COMPANION".to_string());
        companion.retain_source_hierarchy(W3dHierarchy {
            name: "companion".to_string(),
            pivots: vec![
                test_pivot("ROOT", u32::MAX, [0.0; 3]),
                test_pivot("BONE", 0, [4.0, 5.0, 6.0]),
            ],
            pivot_fixups: Vec::new(),
        });
        model.import_named_hierarchy_from(&companion, "COMPANION");
        assert!(model.missing_named_hlod_hierarchy_names().is_empty());

        let (transform, visible) = model
            .mesh_local_transform_and_visibility_for_binding(0, None, 0.0)
            .expect("case-insensitive companion HTree must become the named source");
        assert!(visible);
        let pos = transform.w_axis.truncate();
        assert!(
            (pos - Vec3::new(4.0, 6.0, 5.0)).length() < 0.0001,
            "companion HTree translation, got {pos:?}"
        );
    }

    #[test]
    fn skin_hlod_child_uses_identity_local_after_valid_bind() {
        // C++ `MeshClass::Render` (`mesh.cpp:746-771`) draws SKIN with
        // `Set_World_Identity` after a successful HLOD child create.
        let hierarchy = W3dHierarchy {
            name: "SKIN_HIER".to_string(),
            pivots: vec![
                test_pivot("ROOT", u32::MAX, [0.0; 3]),
                test_pivot("BONE", 0, [10.0, 20.0, 30.0]),
            ],
            pivot_fixups: Vec::new(),
        };
        let mut model = named_hlod_model(hierarchy, 1);
        model.meshes[0].vertex_influences = Some(vec![W3dVertInfStruct {
            bone_idx: 1,
            pad: [0; 6],
        }]);

        let (transform, visible) = model
            .mesh_local_transform_and_visibility_for_binding(0, None, 0.0)
            .expect("a created SKIN LOD child must still be walked");
        assert!(visible);
        assert_eq!(
            transform,
            Mat4::IDENTITY,
            "SKIN mesh-local must stay identity; HTree is the palette"
        );
        assert!(
            W3DModel::mesh_declares_skin(&model.meshes[0]),
            "influence array is enough to stamp HierarchyBindPose"
        );
    }
}
