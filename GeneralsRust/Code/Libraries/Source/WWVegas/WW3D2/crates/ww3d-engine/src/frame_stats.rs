//! Per-frame render statistics (renderer plan step R0).
//!
//! GPU object counts come from wgpu's internal HAL counters (`counters`
//! feature), which track *live* objects. A frame samples them when it opens
//! (`begin_render`) and again right before its single `queue.submit`. Objects
//! created while recording stay alive at least until that submit (the command
//! buffer references them), so the difference is the number of objects the
//! frame created. Frees only happen during device maintenance, which runs at
//! submit, after the second sample.

use std::fmt;
use std::time::Duration;

/// Live wgpu object counts at one instant.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GpuObjectCounts {
    pub buffers: i64,
    pub textures: i64,
    pub texture_views: i64,
    pub bind_groups: i64,
    pub bind_group_layouts: i64,
    pub render_pipelines: i64,
    pub samplers: i64,
    pub shader_modules: i64,
    pub command_encoders: i64,
    pub buffer_memory: i64,
    pub texture_memory: i64,
}

impl GpuObjectCounts {
    /// Sample the device's live object counters.
    pub fn sample(device: &wgpu::Device) -> Self {
        let hal = device.get_internal_counters().hal;
        macro_rules! read {
            ($field:ident) => {
                i64::try_from(hal.$field.read()).unwrap_or(i64::MAX)
            };
        }
        Self {
            buffers: read!(buffers),
            textures: read!(textures),
            texture_views: read!(texture_views),
            bind_groups: read!(bind_groups),
            bind_group_layouts: read!(bind_group_layouts),
            render_pipelines: read!(render_pipelines),
            samplers: read!(samplers),
            shader_modules: read!(shader_modules),
            command_encoders: read!(command_encoders),
            buffer_memory: read!(buffer_memory),
            texture_memory: read!(texture_memory),
        }
    }

    fn minus(self, earlier: Self) -> Self {
        Self {
            buffers: self.buffers - earlier.buffers,
            textures: self.textures - earlier.textures,
            texture_views: self.texture_views - earlier.texture_views,
            bind_groups: self.bind_groups - earlier.bind_groups,
            bind_group_layouts: self.bind_group_layouts - earlier.bind_group_layouts,
            render_pipelines: self.render_pipelines - earlier.render_pipelines,
            samplers: self.samplers - earlier.samplers,
            shader_modules: self.shader_modules - earlier.shader_modules,
            command_encoders: self.command_encoders - earlier.command_encoders,
            buffer_memory: self.buffer_memory - earlier.buffer_memory,
            texture_memory: self.texture_memory - earlier.texture_memory,
        }
    }
}

/// Statistics for one completed engine frame.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RenderFrameStats {
    /// Engine frame index (`WW3D::FrameCount`).
    pub frame_index: u64,
    /// Objects created while the frame was recorded (see module docs).
    pub created: GpuObjectCounts,
    /// Live objects right before submit.
    pub live: GpuObjectCounts,
    /// Render passes the frame recorded through [`crate::RenderFrame::note_render_pass`].
    pub render_passes: u32,
    /// CPU time between `begin_render` and the submit.
    pub record_time: Duration,
}

impl fmt::Display for RenderFrameStats {
    /// One `key=value` line, stable for smoke/golden parsers.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let c = &self.created;
        write!(
            f,
            "frame={} passes={} record_us={} created_buffers={} created_bind_groups={} \
             created_textures={} created_texture_views={} created_pipelines={} \
             created_samplers={} created_shader_modules={} created_encoders={} \
             created_buffer_bytes={} live_buffers={} live_bind_groups={} live_textures={} \
             live_pipelines={} buffer_bytes={} texture_bytes={}",
            self.frame_index,
            self.render_passes,
            self.record_time.as_micros(),
            c.buffers,
            c.bind_groups,
            c.textures,
            c.texture_views,
            c.render_pipelines,
            c.samplers,
            c.shader_modules,
            c.command_encoders,
            c.buffer_memory,
            self.live.buffers,
            self.live.bind_groups,
            self.live.textures,
            self.live.render_pipelines,
            self.live.buffer_memory,
            self.live.texture_memory,
        )
    }
}

/// Counters a frame carries between `begin_render` and `end_render`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct FrameStatsProbe {
    pub(crate) at_begin: GpuObjectCounts,
    pub(crate) render_passes: u32,
}

impl FrameStatsProbe {
    pub(crate) fn begin(device: &wgpu::Device) -> Self {
        Self {
            at_begin: GpuObjectCounts::sample(device),
            render_passes: 0,
        }
    }

    pub(crate) fn finish(
        self,
        device: &wgpu::Device,
        frame_index: u64,
        record_time: Duration,
    ) -> RenderFrameStats {
        let live = GpuObjectCounts::sample(device);
        RenderFrameStats {
            frame_index,
            created: live.minus(self.at_begin),
            live,
            render_passes: self.render_passes,
            record_time,
        }
    }
}
