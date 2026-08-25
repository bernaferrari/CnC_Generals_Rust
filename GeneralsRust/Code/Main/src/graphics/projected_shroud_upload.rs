//! Frozen W3D shroud R8 upload foundation.
//!
//! This layer owns only GPU resource lifetime and byte uploads.  It accepts an
//! immutable [`ProjectedShroudSnapshot`] made during presentation construction;
//! it deliberately has no simulation/FOW query path and does not install a
//! material pass.  The latter belongs to the dependent renderer task.

use crate::fow_rendering::ProjectedShroudSnapshot;
use std::sync::Arc;
use ww3d_renderer_3d::rendering::projected_shroud::{
    FrozenProjectedShroudTexture, ProjectedShroudProjection,
};

/// WGPU format for the source-shaped one-channel shroud level texture.
pub const PROJECTED_SHROUD_TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R8Unorm;

/// Frozen portable mapping for W3D's shroud texture filter state.
///
/// `W3DShroud` uses `TextureFilterClass::FILTER_TYPE_DEFAULT` for min/mag
/// filtering, while its destination texture is explicitly clamp-addressed and
/// has no mip levels.  `FILTER_TYPE_DEFAULT` ultimately depends on legacy D3D
/// capabilities and the user's texture-filter setting, so no WGPU sampler can
/// claim to be an exact driver-independent equivalence.  This foundation
/// records the deliberate port policy separately: the current portable
/// mapping is linear min/mag with clamp addressing and no mip filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectedShroudSamplerPolicy {
    W3dDefaultFilterMapping,
}

impl ProjectedShroudSamplerPolicy {
    #[inline]
    fn wgpu_filters(self) -> (wgpu::FilterMode, wgpu::FilterMode, wgpu::FilterMode) {
        match self {
            Self::W3dDefaultFilterMapping => (
                wgpu::FilterMode::Linear,
                wgpu::FilterMode::Linear,
                // The source destination is MIP_LEVELS_1 and explicitly sets
                // FILTER_TYPE_NONE for mip mapping.  This field is inert with
                // one level, but stays explicit in the frozen policy.
                wgpu::FilterMode::Nearest,
            ),
        }
    }
}

pub const PROJECTED_SHROUD_SAMPLER_POLICY: ProjectedShroudSamplerPolicy =
    ProjectedShroudSamplerPolicy::W3dDefaultFilterMapping;

/// CPU-visible resident texture identity used to decide allocation versus write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectedShroudTextureState {
    pub extent: (u32, u32),
    pub texture_fingerprint: u64,
}

/// The only resource transition permitted for a frozen shroud snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectedShroudUploadAction {
    /// Snapshot is inactive or malformed: release the old texture so a later
    /// pass cannot accidentally sample stale fog.
    Deactivate,
    /// Allocate a new R8 texture, then write the complete frozen payload.
    AllocateAndWrite,
    /// Reuse the existing allocation and write changed R8 bytes.
    Write,
    /// Same allocation and byte fingerprint: preserve the resource unchanged.
    Unchanged,
}

/// Pure upload decision.  Keeping this separate from WGPU makes resize and
/// fingerprint behavior testable without a graphics adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectedShroudUploadPlan {
    pub action: ProjectedShroudUploadAction,
    pub target: Option<ProjectedShroudTextureState>,
}

impl ProjectedShroudUploadPlan {
    pub fn for_snapshot(
        snapshot: &ProjectedShroudSnapshot,
        resident: Option<ProjectedShroudTextureState>,
    ) -> Self {
        let Some(extent) = snapshot.texture_extent() else {
            return Self {
                action: ProjectedShroudUploadAction::Deactivate,
                target: None,
            };
        };
        let target = ProjectedShroudTextureState {
            extent,
            texture_fingerprint: snapshot.texture_fingerprint(),
        };
        let action = match resident {
            None => ProjectedShroudUploadAction::AllocateAndWrite,
            Some(current) if current.extent != target.extent => {
                ProjectedShroudUploadAction::AllocateAndWrite
            }
            Some(current) if current.texture_fingerprint != target.texture_fingerprint => {
                ProjectedShroudUploadAction::Write
            }
            Some(_) => ProjectedShroudUploadAction::Unchanged,
        };
        Self {
            action,
            target: Some(target),
        }
    }
}

struct ProjectedShroudGpuTexture {
    texture: wgpu::Texture,
    view: Arc<wgpu::TextureView>,
    sampler: Arc<wgpu::Sampler>,
}

/// Presentation-owned GPU texture cache for the projected W3D shroud input.
///
/// The texture is full-frame/frozen data, not a live shroud-manager bridge.
/// W3D's destination texture is clamp-addressed (`W3DShroud::ReAcquireResources`),
/// with no mip levels.  Min/mag filtering follows the named frozen
/// [`PROJECTED_SHROUD_SAMPLER_POLICY`], rather than asserting an unconditional
/// exact linear equivalence for every legacy D3D driver.
pub struct ProjectedShroudGpuUploader {
    texture: Option<ProjectedShroudGpuTexture>,
    resident: Option<ProjectedShroudTextureState>,
    allocation_count: u64,
    upload_count: u64,
}

impl Default for ProjectedShroudGpuUploader {
    fn default() -> Self {
        Self {
            texture: None,
            resident: None,
            allocation_count: 0,
            upload_count: 0,
        }
    }
}

impl ProjectedShroudGpuUploader {
    /// Decide and apply the minimal safe GPU transition for this frozen frame.
    ///
    /// `Deactivate` clears both the resident identity and WGPU texture.  A
    /// caller that receives an inactive frame therefore cannot retain an old
    /// map's shroud binding across a reset or shell transition.
    pub fn sync(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        snapshot: &ProjectedShroudSnapshot,
    ) -> ProjectedShroudUploadPlan {
        let plan = ProjectedShroudUploadPlan::for_snapshot(snapshot, self.resident);
        match plan.action {
            ProjectedShroudUploadAction::Deactivate => {
                self.texture = None;
                self.resident = None;
            }
            ProjectedShroudUploadAction::AllocateAndWrite => {
                let target = plan.target.expect("active projection has a texture target");
                self.texture = Some(Self::create_texture(device, target.extent));
                self.resident = Some(target);
                self.allocation_count = self.allocation_count.saturating_add(1);
                self.write_snapshot(queue, snapshot, target.extent);
                self.upload_count = self.upload_count.saturating_add(1);
            }
            ProjectedShroudUploadAction::Write => {
                let target = plan.target.expect("active projection has a texture target");
                self.write_snapshot(queue, snapshot, target.extent);
                self.resident = Some(target);
                self.upload_count = self.upload_count.saturating_add(1);
            }
            ProjectedShroudUploadAction::Unchanged => {}
        }
        plan
    }

    fn create_texture(device: &wgpu::Device, extent: (u32, u32)) -> ProjectedShroudGpuTexture {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Projected W3D shroud R8"),
            size: wgpu::Extent3d {
                width: extent.0,
                height: extent.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: PROJECTED_SHROUD_TEXTURE_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = Arc::new(texture.create_view(&wgpu::TextureViewDescriptor::default()));
        let (mag_filter, min_filter, mipmap_filter) =
            PROJECTED_SHROUD_SAMPLER_POLICY.wgpu_filters();
        let sampler = Arc::new(device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Projected W3D shroud default-filter mapping"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter,
            min_filter,
            mipmap_filter,
            ..Default::default()
        }));
        ProjectedShroudGpuTexture {
            texture,
            view,
            sampler,
        }
    }

    fn write_snapshot(
        &self,
        queue: &wgpu::Queue,
        snapshot: &ProjectedShroudSnapshot,
        extent: (u32, u32),
    ) {
        let texture = self
            .texture
            .as_ref()
            .expect("upload plan cannot write without an allocated texture");
        debug_assert_eq!(snapshot.texture_extent(), Some(extent));
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &snapshot.texels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                // Queue::write_texture accepts unpadded rows.  It performs the
                // staging alignment internally, so exact R8 width is preserved.
                bytes_per_row: Some(extent.0),
                rows_per_image: Some(extent.1),
            },
            wgpu::Extent3d {
                width: extent.0,
                height: extent.1,
                depth_or_array_layers: 1,
            },
        );
    }

    #[inline]
    pub fn texture_view(&self) -> Option<&wgpu::TextureView> {
        self.texture.as_ref().map(|texture| texture.view.as_ref())
    }

    #[inline]
    pub fn sampler(&self) -> Option<&wgpu::Sampler> {
        self.texture
            .as_ref()
            .map(|texture| texture.sampler.as_ref())
    }

    /// Freeze the current uploaded resource together with this frame's exact
    /// C++ projection/tint metadata for renderer ingress. A stale or mismatched
    /// snapshot cannot borrow the resident texture.
    pub fn renderer_binding(
        &self,
        snapshot: &ProjectedShroudSnapshot,
    ) -> Option<FrozenProjectedShroudTexture> {
        let extent = snapshot.texture_extent()?;
        let resident = self.resident?;
        if resident.extent != extent
            || resident.texture_fingerprint != snapshot.texture_fingerprint()
        {
            return None;
        }
        let texture = self.texture.as_ref()?;
        let projection = ProjectedShroudProjection::from_cpp_grid(
            snapshot.draw_origin_xy,
            snapshot.cell_size_xy,
            extent,
            snapshot.metadata.shroud_color_rgb,
            snapshot.content_fingerprint(),
        )?;
        Some(FrozenProjectedShroudTexture::new(
            Arc::clone(&texture.view),
            Arc::clone(&texture.sampler),
            projection,
        ))
    }

    #[inline]
    pub fn resident_state(&self) -> Option<ProjectedShroudTextureState> {
        self.resident
    }

    #[inline]
    pub fn allocation_count(&self) -> u64 {
        self.allocation_count
    }

    #[inline]
    pub fn upload_count(&self) -> u64 {
        self.upload_count
    }

    /// Explicit resource release for renderer/device teardown.
    pub fn clear(&mut self) {
        self.texture = None;
        self.resident = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fow_rendering::{PresentationFowGrid, ProjectedShroudMetadata};

    fn snapshot(width: u32, height: u32, cells: Vec<u8>) -> ProjectedShroudSnapshot {
        let grid = PresentationFowGrid::from_snapshot(width, height, 10.0, cells);
        ProjectedShroudSnapshot::from_grid(&grid, ProjectedShroudMetadata::default())
    }

    #[test]
    fn upload_plan_allocates_then_skips_identical_frozen_r8() {
        let snapshot = snapshot(
            2,
            1,
            vec![
                PresentationFowGrid::CELL_VISIBLE,
                PresentationFowGrid::CELL_EXPLORED,
            ],
        );
        let first = ProjectedShroudUploadPlan::for_snapshot(&snapshot, None);
        assert_eq!(first.action, ProjectedShroudUploadAction::AllocateAndWrite);
        assert_eq!(first.target.unwrap().extent, (4, 4));

        let second = ProjectedShroudUploadPlan::for_snapshot(&snapshot, first.target);
        assert_eq!(second.action, ProjectedShroudUploadAction::Unchanged);
    }

    #[test]
    fn upload_plan_writes_changed_bytes_but_reallocates_on_resize() {
        let first = snapshot(
            2,
            1,
            vec![
                PresentationFowGrid::CELL_VISIBLE,
                PresentationFowGrid::CELL_EXPLORED,
            ],
        );
        let resident = ProjectedShroudUploadPlan::for_snapshot(&first, None)
            .target
            .expect("initial target");

        let changed = snapshot(
            2,
            1,
            vec![
                PresentationFowGrid::CELL_VISIBLE,
                PresentationFowGrid::CELL_HIDDEN,
            ],
        );
        let changed_plan = ProjectedShroudUploadPlan::for_snapshot(&changed, Some(resident));
        assert_eq!(changed_plan.action, ProjectedShroudUploadAction::Write);

        let resized = snapshot(
            3,
            1,
            vec![
                PresentationFowGrid::CELL_VISIBLE,
                PresentationFowGrid::CELL_EXPLORED,
                PresentationFowGrid::CELL_HIDDEN,
            ],
        );
        let resized_plan = ProjectedShroudUploadPlan::for_snapshot(&resized, Some(resident));
        assert_eq!(
            resized_plan.action,
            ProjectedShroudUploadAction::AllocateAndWrite
        );
        assert_eq!(resized_plan.target.unwrap().extent, (8, 4));
    }

    #[test]
    fn upload_plan_releases_stale_resource_for_inactive_snapshot() {
        let active = snapshot(1, 1, vec![PresentationFowGrid::CELL_VISIBLE]);
        let resident = ProjectedShroudUploadPlan::for_snapshot(&active, None)
            .target
            .expect("active target");
        let plan = ProjectedShroudUploadPlan::for_snapshot(
            &ProjectedShroudSnapshot::inactive(),
            Some(resident),
        );
        assert_eq!(plan.action, ProjectedShroudUploadAction::Deactivate);
        assert_eq!(plan.target, None);
    }

    #[test]
    fn projected_shroud_gpu_upload_never_imports_live_game_logic() {
        let source = include_str!("projected_shroud_upload.rs");
        for forbidden_use in ["use crate::game_logic", "use gamelogic"] {
            assert!(
                source
                    .lines()
                    .all(|line| !line.trim_start().starts_with(forbidden_use)),
                "GPU upload must only consume frozen presentation data: {forbidden_use}"
            );
        }
    }

    #[test]
    fn projected_shroud_resource_and_exact_mesh_eligibility_reach_renderer_ingress() {
        let execute = include_str!("render_pipeline/pipeline_execute.rs");
        let forward = include_str!("render_pipeline/forward_render.rs");
        assert!(execute.contains("frame.terrain_projected_shroud()"));
        assert!(forward.contains("projected_shroud_uploader.sync("));
        assert!(forward.contains(".renderer_binding(projected_shroud)"));
        assert!(forward.contains("renderer.set_projected_shroud(projected_shroud_binding)"));
        assert!(
            forward.contains(
                "mesh.set_projected_shroud_eligible(item.pushes_projected_shroud_pass())"
            )
        );
        assert!(!forward.contains("get_shroud_manager"));
    }
}
