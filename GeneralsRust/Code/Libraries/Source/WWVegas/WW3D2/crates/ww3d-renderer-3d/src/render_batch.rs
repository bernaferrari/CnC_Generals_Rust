//! Draw an already-prepared object after the viewport filter, in this frame.
//! W3DView.cpp1560–1574 redraws the locked drawable without another scene update.
use super::*;
use crate::rendering::frame_graph::FrameGraphNode;
use crate::rendering::mesh_system::MeshClass;

impl Renderer {
    /// Composite a caller-owned batch using the receiving renderer's current
    /// camera and lighting. Existing color and depth are loaded; animation,
    /// frame queues, shadows, and the frame arena are not advanced or reset.
    ///
    /// Call only within the current engine-frame callback, after its filter and
    /// camera continuation. The caller prepares geometry once and decides which
    /// objects must be excluded from the preceding scene pass.
    pub fn render_mesh_batch_into_frame(
        &mut self,
        frame: &mut ww3d_engine::RenderFrame,
        meshes: &[Arc<MeshClass>],
    ) -> RendererResult<()> {
        if meshes.is_empty() {
            return Ok(());
        }
        let mut info = self.build_render_info()?;
        info.time = self.animation_coordinator.clock().elapsed_seconds();
        info.frame_count = self.animation_coordinator.frame_number() as u64;
        let mut batch = FrameGraphNode::new(FrameGraphPass::Custom(0));
        batch.set_context(FrameGraphPassContext::from_render_info(
            FrameGraphPass::Custom(0),
            &info,
        ));
        for mesh in meshes {
            if let Some(model) = &mesh.model {
                self.register_mesh(model.clone())?;
            }
            let queue = if mesh.is_decal_instance {
                FrameGraphQueue::Decal
            } else if mesh.is_alpha() {
                FrameGraphQueue::Alpha
            } else {
                FrameGraphQueue::Opaque
            };
            batch.submit_mesh(mesh.clone(), Some(queue), None);
        }
        let prepared = batch.prepare(info.camera.get_position());
        let mut translucent = prepared.combined_translucent();
        translucent.extend(prepared.decals.iter().cloned());
        let arena = self
            .frame_uniform_arenas
            .get_mut(self.frame_uniform_index)
            .ok_or_else(|| W3DError::NotInitialized("frame uniform arenas uninitialised".into()))?;
        let color = frame.color_view_arc();
        let depth = frame.depth_view_arc();
        let size = frame.size();
        {
            let mut pass = frame
                .encoder()
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("WW3D Post-Filter Object Batch"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: color.as_ref(),
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: depth.as_deref().map(|view| {
                        wgpu::RenderPassDepthStencilAttachment {
                            view,
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            }),
                            stencil_ops: None,
                        }
                    }),
                    occlusion_query_set: None,
                    timestamp_writes: None,
                    multiview_mask: None,
                });
            set_batch_viewport(&mut pass, &info.camera, size);
            self.mesh_render_manager.render_pass(
                &mut pass,
                &prepared.opaque,
                &translucent,
                &info,
                arena,
            )?;
        }
        // Sorted polygon materials submit into the existing backend sorter.
        // Flush this batch before the caller paints UI, using load attachments.
        if ww3d_gpu::sorting_renderer_stats().batch_count > 0 {
            let mut pass = frame
                .encoder()
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("WW3D Post-Filter Object Sorting Flush"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: color.as_ref(),
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: depth.as_deref().map(|view| {
                        wgpu::RenderPassDepthStencilAttachment {
                            view,
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            }),
                            stencil_ops: None,
                        }
                    }),
                    occlusion_query_set: None,
                    timestamp_writes: None,
                    multiview_mask: None,
                });
            set_batch_viewport(&mut pass, &info.camera, size);
            ww3d_gpu::flush_sorting_renderer(&mut pass);
        }
        Ok(())
    }
}

fn set_batch_viewport(
    pass: &mut wgpu::RenderPass<'_>,
    camera: &rendering::camera_system::CameraClass,
    (width, height): (u32, u32),
) {
    if width == 0 || height == 0 {
        return;
    }
    let viewport = camera.get_viewport();
    let x = viewport.min.x * width as f32;
    let y = viewport.min.y * height as f32;
    let w = ((viewport.max.x - viewport.min.x) * width as f32).max(1.0);
    let h = ((viewport.max.y - viewport.min.y) * height as f32).max(1.0);
    pass.set_viewport(x, y, w, h, 0.0, 1.0);
    let sx = (x as u32).min(width - 1);
    let sy = (y as u32).min(height - 1);
    pass.set_scissor_rect(
        sx,
        sy,
        (w as u32).clamp(1, width - sx),
        (h as u32).clamp(1, height - sy),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rendering::camera_system::CameraClass;
    use crate::rendering::mesh_system::MeshModelClass;
    use ww3d_core::w3d_format::{W3dTriangleStruct, W3dVectorStruct};

    #[test]
    #[ignore = "requires a native GPU; run explicitly with --include-ignored"]
    fn post_filter_batch_loads_color_and_depth_without_advancing_animation() {
        ww3d_engine::init_headless_blocking(ww3d_engine::EngineConfig {
            width: 32,
            height: 32,
            ..Default::default()
        })
        .expect("headless renderer");
        let device = ww3d_engine::device().unwrap();
        let mut renderer = Renderer::new(ww3d_engine::gpu_device().unwrap());
        renderer.mesh_render_manager.set_render_formats(
            ww3d_engine::color_format().unwrap(),
            ww3d_engine::depth_format().unwrap(),
        );
        let mut model = MeshModelClass::new("post-filter-load-witness");
        model.vertices = [(-0.8, -0.8), (0.8, -0.8), (0.0, 0.8)]
            .into_iter()
            .map(|(x, y)| W3dVectorStruct { x, y, z: 0.5 })
            .collect();
        for vindex in [[0, 1, 2], [0, 2, 1]] {
            model.triangles.push(W3dTriangleStruct {
                vindex,
                attributes: 0,
                normal: W3dVectorStruct {
                    x: 0.0,
                    y: 0.0,
                    z: 1.0,
                },
                distance: 0.0,
            });
        }
        model.vertex_count = 3;
        model.index_count = 6;
        let mut mesh = MeshClass::new();
        mesh.model = Some(Arc::new(model));
        let batch = [Arc::new(mesh)];
        let mut pixels = Vec::new();
        for depth_clear in [0.0, 1.0] {
            renderer.begin_frame().unwrap();
            let mut camera = CameraClass::new();
            camera.set_view_matrix(glam::Mat4::IDENTITY);
            camera.set_projection_matrix(glam::Mat4::IDENTITY);
            renderer.set_camera(camera);
            let mut frame = ww3d_engine::begin_render().unwrap();
            renderer
                .render_frame(&mut frame, Some(wgpu::Color::RED), None)
                .unwrap();
            let time = renderer.get_animation_time();
            let color = frame.color_view_arc();
            let depth = frame.depth_view_arc();
            {
                // Stand in for the completed scene/filter; the redraw must
                // retain both this color and the scene's selected depth.
                let _pass = frame
                    .encoder()
                    .begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("post-filter-test-seed"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: color.as_ref(),
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::RED),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: depth.as_deref().map(|view| {
                            wgpu::RenderPassDepthStencilAttachment {
                                view,
                                depth_ops: Some(wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(depth_clear),
                                    store: wgpu::StoreOp::Store,
                                }),
                                stencil_ops: None,
                            }
                        }),
                        occlusion_query_set: None,
                        timestamp_writes: None,
                        multiview_mask: None,
                    });
            }
            renderer
                .render_mesh_batch_into_frame(&mut frame, &batch)
                .unwrap();
            assert_eq!(renderer.get_animation_time().to_bits(), time.to_bits());
            let readback = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("post-filter-batch-readback"),
                size: 256 * 32,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            let (texture, encoder) = frame.color_texture_and_encoder();
            encoder.copy_texture_to_buffer(
                texture.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer: &readback,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(256),
                        rows_per_image: Some(32),
                    },
                },
                wgpu::Extent3d {
                    width: 32,
                    height: 32,
                    depth_or_array_layers: 1,
                },
            );
            ww3d_engine::end_render(frame).unwrap();
            let slice = readback.slice(..);
            let (tx, rx) = std::sync::mpsc::channel();
            slice.map_async(wgpu::MapMode::Read, move |result| {
                tx.send(result).unwrap();
            });
            device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            rx.recv().unwrap().unwrap();
            let bytes = slice.get_mapped_range().unwrap();
            pixels.push((
                bytes[16 * 256 + 16 * 4..16 * 256 + 16 * 4 + 4].to_vec(),
                bytes[0..4].to_vec(),
            ));
            drop(bytes);
            readback.unmap();
        }
        assert_eq!(
            pixels[0].0, pixels[0].1,
            "loaded near scene depth occludes redraw"
        );
        assert_eq!(
            pixels[0].1, pixels[1].1,
            "batch never clears untouched scene color"
        );
        assert_ne!(
            pixels[1].0, pixels[1].1,
            "far scene depth admits the actual mesh draw"
        );
    }
}
