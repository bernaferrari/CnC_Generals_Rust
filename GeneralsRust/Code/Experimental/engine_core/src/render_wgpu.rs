use anyhow::Result;

use crate::EngineSnapshot;

pub struct WgpuRenderer;

impl Default for WgpuRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl WgpuRenderer {
    pub fn new() -> Self {
        Self
    }

    pub fn render_snapshot(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target_view: &wgpu::TextureView,
        snapshot: &EngineSnapshot,
    ) -> Result<()> {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("experimental-engine-render"),
        });

        let color = wgpu::Color {
            r: snapshot.clear_color[0] as f64,
            g: snapshot.clear_color[1] as f64,
            b: snapshot.clear_color[2] as f64,
            a: 1.0,
        };

        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("experimental-engine-clear-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
        }

        queue.submit(std::iter::once(encoder.finish()));
        Ok(())
    }
}
