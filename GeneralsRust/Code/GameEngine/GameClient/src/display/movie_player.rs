use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use crate::bink::BinkDecoder;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoviePlaybackState {
    Stopped,
    Playing,
    Paused,
    Finished,
}

pub struct WgpuBinkVideoPlayer {
    device: Arc<wgpu::Device>,
    surface_format: wgpu::TextureFormat,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    pipeline: wgpu::RenderPipeline,
    decoder: Option<BinkDecoder>,
    texture: Option<wgpu::Texture>,
    texture_view: Option<wgpu::TextureView>,
    bind_group: Option<wgpu::BindGroup>,
    state: MoviePlaybackState,
    frame_accumulator: Duration,
    current_frame_rgba: Vec<u8>,
}

impl WgpuBinkVideoPlayer {
    pub fn new(device: Arc<wgpu::Device>, surface_format: wgpu::TextureFormat) -> Self {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Movie Player Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Movie Player Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Movie Player Shader"),
            source: wgpu::ShaderSource::Wgsl(MOVIE_PLAYER_SHADER.into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Movie Player Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Movie Player Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            device,
            surface_format,
            bind_group_layout,
            sampler,
            pipeline,
            decoder: None,
            texture: None,
            texture_view: None,
            bind_group: None,
            state: MoviePlaybackState::Stopped,
            frame_accumulator: Duration::ZERO,
            current_frame_rgba: Vec::new(),
        }
    }

    pub fn open(&mut self, path: &Path) -> Result<(), String> {
        let mut decoder = BinkDecoder::open(path)?;
        self.create_texture_resources(decoder.width(), decoder.height());
        self.current_frame_rgba = decoder.decode_current_frame_rgba();
        self.decoder = Some(decoder);
        self.state = MoviePlaybackState::Paused;
        self.frame_accumulator = Duration::ZERO;
        Ok(())
    }

    pub fn play(&mut self) {
        if self.decoder.is_some() {
            self.state = MoviePlaybackState::Playing;
        }
    }

    pub fn pause(&mut self) {
        if self.state == MoviePlaybackState::Playing {
            self.state = MoviePlaybackState::Paused;
        }
    }

    pub fn stop(&mut self) {
        self.state = MoviePlaybackState::Stopped;
        self.decoder = None;
        self.texture = None;
        self.texture_view = None;
        self.bind_group = None;
        self.current_frame_rgba.clear();
        self.frame_accumulator = Duration::ZERO;
    }

    pub fn update(&mut self, dt: Duration, queue: &wgpu::Queue) {
        let Some(decoder) = self.decoder.as_ref() else {
            return;
        };

        let width = decoder.width();
        let height = decoder.height();
        let frame_duration = decoder.frame_duration();

        if self.texture.is_none() {
            self.create_texture_resources(width, height);
        }

        if !self.current_frame_rgba.is_empty() {
            self.upload_current_frame(queue, width, height);
        }

        if self.state != MoviePlaybackState::Playing {
            return;
        }

        self.frame_accumulator += dt;

        while self.frame_accumulator >= frame_duration {
            self.frame_accumulator -= frame_duration;
            let reached_end = self
                .decoder
                .as_ref()
                .map(|decoder| decoder.current_frame_index() + 1 >= decoder.frame_count())
                .unwrap_or(true);
            if reached_end {
                self.state = MoviePlaybackState::Finished;
                break;
            }

            let next_frame = {
                let decoder = self
                    .decoder
                    .as_mut()
                    .expect("decoder must exist while updating");
                decoder.advance();
                decoder.decode_current_frame_rgba()
            };
            self.current_frame_rgba = next_frame;
            self.upload_current_frame(queue, width, height);
        }
    }

    pub fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target_view: &wgpu::TextureView,
        target_width: u32,
        target_height: u32,
    ) {
        let Some(bind_group) = self.bind_group.as_ref() else {
            return;
        };
        let Some(decoder) = self.decoder.as_ref() else {
            return;
        };

        let (viewport_x, viewport_y, viewport_width, viewport_height) = movie_viewport(
            target_width.max(1),
            target_height.max(1),
            decoder.width().max(1),
            decoder.height().max(1),
        );

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Fullscreen Movie Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.set_viewport(
            viewport_x,
            viewport_y,
            viewport_width.max(1.0),
            viewport_height.max(1.0),
            0.0,
            1.0,
        );
        pass.draw(0..3, 0..1);
    }

    pub fn is_playing(&self) -> bool {
        self.state == MoviePlaybackState::Playing
    }

    pub fn is_finished(&self) -> bool {
        self.state == MoviePlaybackState::Finished
    }

    pub fn get_texture(&self) -> Option<&wgpu::TextureView> {
        self.texture_view.as_ref()
    }

    fn create_texture_resources(&mut self, width: u32, height: u32) {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Fullscreen Movie Texture"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Movie Player Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        self.texture = Some(texture);
        self.texture_view = Some(texture_view);
        self.bind_group = Some(bind_group);
    }

    fn upload_current_frame(&self, queue: &wgpu::Queue, width: u32, height: u32) {
        let Some(texture) = self.texture.as_ref() else {
            return;
        };
        if self.current_frame_rgba.is_empty() {
            return;
        }

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &self.current_frame_rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
    }
}

fn movie_viewport(
    target_width: u32,
    target_height: u32,
    movie_width: u32,
    movie_height: u32,
) -> (f32, f32, f32, f32) {
    let target_aspect = target_width as f32 / target_height.max(1) as f32;
    let movie_aspect = movie_width as f32 / movie_height.max(1) as f32;

    if movie_aspect > target_aspect {
        let viewport_width = target_width as f32;
        let viewport_height = viewport_width / movie_aspect;
        let viewport_y = ((target_height as f32 - viewport_height) * 0.5).max(0.0);
        (0.0, viewport_y, viewport_width, viewport_height)
    } else {
        let viewport_height = target_height as f32;
        let viewport_width = viewport_height * movie_aspect;
        let viewport_x = ((target_width as f32 - viewport_width) * 0.5).max(0.0);
        (viewport_x, 0.0, viewport_width, viewport_height)
    }
}

const MOVIE_PLAYER_SHADER: &str = r#"
struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@group(0) @binding(0) var movie_texture: texture_2d<f32>;
@group(0) @binding(1) var movie_sampler: sampler;

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VsOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 3.0,  1.0),
    );
    var uvs = array<vec2<f32>, 3>(
        vec2<f32>(0.0, 2.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(2.0, 0.0),
    );
    var out: VsOut;
    out.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    out.uv = uvs[vertex_index];
    return out;
}

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4<f32> {
    return textureSample(movie_texture, movie_sampler, input.uv);
}
"#;

#[cfg(test)]
mod tests {
    use super::movie_viewport;

    #[test]
    fn preserves_movie_aspect_ratio() {
        let (x, y, w, h) = movie_viewport(1920, 1080, 720, 486);
        assert!(x >= 0.0);
        assert!(y >= 0.0);
        assert!(w <= 1920.0);
        assert!(h <= 1080.0);
    }
}
