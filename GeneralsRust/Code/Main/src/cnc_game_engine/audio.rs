#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;
impl CnCGameEngine {
    pub(super) fn toggle_pause(&mut self) {
        // C++ has no dedicated pause screen: in-match pause is ESC /
        // MSG_META_OPTIONS -> ToggleQuitMenu (CommandXlat.cpp:3091-3094,
        // QuitMenu.cpp:260) — the pause is a side effect of the live
        // QuitMenu WND being visible. Route the host pause to that same
        // retail projection; the legacy software PauseMenu remains only as
        // the damaged-install fallback for a failed WND toggle.
        if self.host_toggle_retail_quit_menu() {
            info!(
                "Game {}",
                if self.game_paused {
                    "PAUSED"
                } else {
                    "RESUMED"
                }
            );
            return;
        }

        self.host_set_paused(!self.game_paused);

        info!(
            "Game {}",
            if self.game_paused {
                "PAUSED"
            } else {
                "RESUMED"
            }
        );

        // Notify UI
        self.ui_manager.queue_event(if self.game_paused {
            UIEvent::ChangeScreen(Screen::PauseMenu)
        } else {
            UIEvent::ChangeScreen(Screen::GameHUD)
        });
    }

    pub(super) fn start_background_music(&mut self) {
        let handle = match &self.audio_handle {
            Some(handle) => handle,
            None => {
                info!("Background music skipped (-noaudio)");
                return;
            }
        };

        let sink = match Sink::try_new(handle) {
            Ok(sink) => sink,
            Err(err) => {
                error!("Failed to create music sink: {err}");
                return;
            }
        };

        // Create ambient RTS music
        let sample_rate = 44_100;
        let duration = 30.0; // 30 second loop
        let samples: Vec<f32> = (0..(sample_rate as f32 * duration) as usize)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                let base = (t * 220.0 * 2.0 * std::f32::consts::PI).sin() * 0.05;
                let harmony1 = (t * 330.0 * 2.0 * std::f32::consts::PI).sin() * 0.03;
                let harmony2 = (t * 440.0 * 2.0 * std::f32::consts::PI).sin() * 0.02;
                base + harmony1 + harmony2
            })
            .collect();

        let source = rodio::buffer::SamplesBuffer::new(1, sample_rate, samples).repeat_infinite();
        sink.append(source);

        self.background_music = Some(sink);
        info!("Background music started");
    }

    pub(super) fn toggle_background_music(&mut self) {
        if self.audio_handle.is_none() {
            info!("Background music unavailable (-noaudio)");
            return;
        }

        if let Some(music) = &self.background_music {
            if music.is_paused() {
                music.play();
                info!("Background music resumed");
            } else {
                music.pause();
                info!("Background music paused");
            }
        } else {
            // DISABLED: Using proper AssetManager audio system instead of synthetic tones
            // self.start_background_music();
            info!("Background music would be started, but synthetic audio is disabled");
        }
    }

    /// Wave 604: via `host_play_sound_effect`.
    pub(super) fn play_sound_effect(&mut self, sound_type: SoundType) {
        // Wave 604: thin wrapper — UI SFX via host helper.
        self.host_play_sound_effect(sound_type);
    }

    pub(super) fn host_play_sound_effect(&mut self, sound_type: SoundType) {
        // Wave 604: host UI SFX residual.
        // C++ `TheAudio->addAudioEvent` (`AudioManager::addAudioEvent`, GameAudio.cpp).
        // One play API: Common TheAudio / AudioManager when a live handle exists.
        // C++ CommandXlat.cpp:271-731 pickAndPlayUnitVoiceResponse — VoiceSelect /
        // VoiceMove / VoiceAttack from ThingTemplate. Never invent UnitSelect or
        // UnitCommand (those tokens are not in MiscAudio.ini / Voice.ini).
        if matches!(sound_type, SoundType::Select | SoundType::Command) {
            return;
        }
        let kind = match sound_type {
            SoundType::Select | SoundType::Command => return,
            SoundType::ConstructionComplete => "ConstructionComplete",
            SoundType::UnitReady => "UnitReady",
            SoundType::UpgradeComplete => "UpgradeComplete",
            SoundType::Hit => "WeaponHit",
            SoundType::Explosion => "Explosion",
            SoundType::Build => "BuildingComplete",
        };
        if crate::assets::audio::play_sound_through_the_audio(kind).is_some() {
            log::trace!("🔊 UI SFX via TheAudio: {kind}");
            return;
        }

        // Prefer presentation/host audio event residual when a frame is installed
        // (InGame path). Avoid dual synthetic rodio tones competing with event queue.
        if self.last_presentation_frame.is_some() {
            // Presentation path: leftover AudioManagerSubsystem queue only if
            // TheAudio produced no live handle. No GameLogic dual-write.
            let event = crate::game_logic::AudioEventRequest::new(kind);
            log::trace!("🔊 UI presentation audio: {}", event.event_type);
            let _ = crate::subsystem_manager::with_subsystem_mut::<
                crate::subsystem_manager::AudioManagerSubsystem,
                _,
            >(|audio| audio.queue_event(event));
            return;
        }

        // Boot residual only — synthetic tones when no TheAudio handle.
        let handle = match &self.audio_handle {
            Some(handle) => handle,
            None => {
                return;
            }
        };

        let sink = match Sink::try_new(handle) {
            Ok(sink) => sink,
            Err(err) => {
                error!("Failed to create sound effect sink: {err}");
                return;
            }
        };

        let (frequency, duration) = match sound_type {
            SoundType::Select | SoundType::Command => return,
            SoundType::ConstructionComplete => (900.0, 0.25),
            SoundType::UnitReady => (700.0, 0.2),
            SoundType::UpgradeComplete => (750.0, 0.22),
            SoundType::Hit => (300.0, 0.2),
            SoundType::Explosion => (150.0, 0.5),
            SoundType::Build => (1000.0, 0.3),
        };

        let sample_rate = 44_100;
        let samples: Vec<f32> = (0..(sample_rate as f32 * duration) as usize)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                let envelope = 1.0 - (t / duration); // Fade out
                (t * frequency * 2.0 * std::f32::consts::PI).sin() * 0.2 * envelope
            })
            .collect();

        let source = rodio::buffer::SamplesBuffer::new(1, sample_rate, samples);
        sink.append(source);
        self.sound_effects.push(sink);
    }

    pub(super) fn cleanup_sound_effects(&mut self) {
        self.sound_effects.retain(|sink| !sink.empty());
    }

    /// C++ `GameEngine::update` `TheAudio->UPDATE()` (`GameEngine.cpp:736`).
    /// Live Common `AudioManager::update` drains AR_Play / fade / playing lists.
    pub(super) fn host_update_the_audio(&mut self) {
        self.sync_audio_listener_from_main_camera();
        if let Some(audio) = gamelogic::helpers::TheAudio::get() {
            audio.update();
        }
        // C++ GameEngine.cpp:738 TheGameClient->UPDATE() includes Eva::update
        // after TheAudio. Publish host frame first so Eva.cpp:271 is not 0.
        self.publish_eva_host_frame_and_tick();
    }

    /// C++ GameAudio.cpp:281-330: mic/listener from TheTacticalView camera.
    /// Drive GameClient View so AUDIO_VIEW_RESOLVER is not stuck at (0,0,0).
    pub(super) fn sync_audio_listener_from_main_camera(&self) {
        #[cfg(feature = "game_client")]
        {
            use game_client::display::view::{Point3, with_tactical_view};

            // Main camera is Y-up; GameClient View / C++ Coord3D is Z-up (X/Y ground).
            let target = Point3::new(
                self.camera_target.x,
                self.camera_target.z,
                self.camera_target.y,
            );
            // C++ look_to = Rotate_Z(angle) * (0,1,0) = (-sin(a), cos(a)).
            // Match heading of camera → look-at on the ground plane.
            let look_x = self.camera_target.x - self.camera_position.x;
            let look_y = self.camera_target.z - self.camera_position.z;
            let angle = f32::atan2(-look_x, look_y);
            let zoom = if self.camera_zoom.is_finite() {
                self.camera_zoom.max(0.05)
            } else {
                1.0
            };

            with_tactical_view(|view| {
                view.set_position(&target);
                view.set_angle(angle);
                view.set_zoom(zoom);
                view.init_height_for_map();
            });
        }
    }

    /// Push live host/presentation frame into Eva and run Eva.cpp:264.
    pub(super) fn publish_eva_host_frame_and_tick(&self) {
        #[cfg(feature = "game_client")]
        {
            let frame = self
                .last_presentation_frame
                .as_ref()
                .map(|pres| pres.frame.0)
                .filter(|frame| *frame != 0)
                // Wave 560 fail-closed: boot residual frame, no live dual-read.
                .unwrap_or_else(|| self.presentation_or_boot_logic_frame());
            game_client::eva::set_eva_host_frame(frame);
            // C++ Eva.cpp:422 polls local Energy::hasSufficientPower.
            // Wave 560 fail-closed: boot residual frame, no live dual-read.
            let host_frame = self.presentation_or_boot_logic_frame();
            if let Some(player) = self
                .game_logic
                .local_player_id()
                .and_then(|id| self.game_logic.get_player(id))
                .filter(|p| p.is_alive)
            {
                let sabotaged = host_frame < player.power_sabotaged_till_frame;
                game_client::eva::set_eva_host_sufficient_power(
                    player.power_available >= 0 && !sabotaged,
                );
            } else {
                game_client::eva::clear_eva_host_sufficient_power();
            }
            game_client::eva::update_eva_system();
        }
    }

    /// Get or create a texture bind group for a material (delegated to graphics system)
    pub(super) fn get_material_bind_group(
        &mut self,
        material: &crate::assets::W3DMaterial,
    ) -> Option<&wgpu::BindGroup> {
        // Delegate to graphics system which handles material bind group management
        self.graphics_system.get_material_bind_group(material)
    }

    /// Async texture loading method (for future implementation)
    /// This would be called from a background thread to load textures from BIG archives
    pub(super) async fn load_texture_async(
        &mut self,
        texture_name: &str,
        material_name: &str,
    ) -> Result<(), String> {
        // Texture loading is now handled by the graphics system
        // This method is kept for future implementation of async texture streaming
        println!(
            "🎨 Async texture loading requested for: {} ({})",
            texture_name, material_name
        );
        println!("   (Currently handled by graphics system material management)");
        Ok(())
    }

    /// Legacy fallback cube creation using raw wgpu buffers.
    /// This is now superseded by GraphicsSystem::create_fallback_cube_model() which
    /// creates a W3DModel-based fallback cube cached in the model cache and used by
    /// RenderPipeline::collect_render_items() for objects with missing W3D assets.
    #[allow(dead_code)] // Legacy stub: superseded by GraphicsSystem, retained for reference
    pub(super) fn create_fallback_cube(device: &wgpu::Device) -> (wgpu::Buffer, wgpu::Buffer, u32) {
        // C++ SAGE compatible cube vertices using VertexFormatXYZNDUV2
        let vertices = vec![
            // Front face
            VertexXYZNDUV2 {
                position: [-2.5, -2.5, 2.5],
                normal: [0.0, 0.0, 1.0],
                diffuse: 0xFF0000FF,
                tex_coords0: [0.0, 0.0],
                tex_coords1: [0.0, 0.0],
            }, // Red
            VertexXYZNDUV2 {
                position: [2.5, -2.5, 2.5],
                normal: [0.0, 0.0, 1.0],
                diffuse: 0xFF00FF00,
                tex_coords0: [1.0, 0.0],
                tex_coords1: [1.0, 0.0],
            }, // Green
            VertexXYZNDUV2 {
                position: [2.5, 2.5, 2.5],
                normal: [0.0, 0.0, 1.0],
                diffuse: 0xFFFF0000,
                tex_coords0: [1.0, 1.0],
                tex_coords1: [1.0, 1.0],
            }, // Blue
            VertexXYZNDUV2 {
                position: [-2.5, 2.5, 2.5],
                normal: [0.0, 0.0, 1.0],
                diffuse: 0xFF00FFFF,
                tex_coords0: [0.0, 1.0],
                tex_coords1: [0.0, 1.0],
            }, // Yellow
            // Back face
            VertexXYZNDUV2 {
                position: [-2.5, -2.5, -2.5],
                normal: [0.0, 0.0, -1.0],
                diffuse: 0xFFFF00FF,
                tex_coords0: [0.0, 0.0],
                tex_coords1: [0.0, 0.0],
            }, // Magenta
            VertexXYZNDUV2 {
                position: [2.5, -2.5, -2.5],
                normal: [0.0, 0.0, -1.0],
                diffuse: 0xFFFFFF00,
                tex_coords0: [1.0, 0.0],
                tex_coords1: [1.0, 0.0],
            }, // Cyan
            VertexXYZNDUV2 {
                position: [2.5, 2.5, -2.5],
                normal: [0.0, 0.0, -1.0],
                diffuse: 0xFFFFFFFF,
                tex_coords0: [1.0, 1.0],
                tex_coords1: [1.0, 1.0],
            }, // White
            VertexXYZNDUV2 {
                position: [-2.5, 2.5, -2.5],
                normal: [0.0, 0.0, -1.0],
                diffuse: 0xFF808080,
                tex_coords0: [0.0, 1.0],
                tex_coords1: [0.0, 1.0],
            }, // Gray
        ];

        let indices: Vec<u16> = vec![
            0, 1, 2, 2, 3, 0, // Front
            4, 5, 6, 6, 7, 4, // Back
            7, 3, 0, 0, 4, 7, // Left
            1, 5, 6, 6, 2, 1, // Right
            3, 2, 6, 6, 7, 3, // Top
            0, 1, 5, 5, 4, 0, // Bottom
        ];

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Fallback Cube Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Fallback Cube Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        (vertex_buffer, index_buffer, indices.len() as u32)
    }

    /// C++ SAGE D3D8-style shader - matches original VertexFormatXYZNDUV2 and lighting model
    pub fn get_shader_source() -> &'static str {
        r#"
// C++ SAGE GlobalUniforms equivalent
struct SAGEUniforms {
    view_projection: mat4x4<f32>,
    view_matrix: mat4x4<f32>,
    projection_matrix: mat4x4<f32>,
    camera_position: vec4<f32>,
    time: f32,
    ambient_light: vec3<f32>,
    sun_direction: vec3<f32>,
    sun_color: vec3<f32>,
    _padding: f32,
}

// C++ SAGE MaterialProperties equivalent
struct MaterialProperties {
    diffuse_color: vec4<f32>,
    specular_color: vec4<f32>,
    emissive_color: vec4<f32>,
    opacity: f32,
    shininess: f32,
    stage0_uv_scale: vec2<f32>,
    stage1_uv_scale: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> sage_uniforms: SAGEUniforms;

@group(1) @binding(0)
var stage0_texture: texture_2d<f32>;  // Primary diffuse texture (stage 0)
@group(1) @binding(1)
var stage0_sampler: sampler;

@group(2) @binding(0)
var<uniform> material_properties: MaterialProperties;

// C++ SAGE VertexFormatXYZNDUV2 input - matches D3DVERTEXELEMENT9 declarations
struct VertexInput {
    @location(0) position: vec3<f32>,     // XYZ position
    @location(1) normal: vec3<f32>,       // Normal vector
    @location(2) diffuse: vec4<f32>,      // Diffuse color (unpacked from u32)
    @location(3) tex_coords0: vec2<f32>,  // Primary UV coordinates
    @location(4) tex_coords1: vec2<f32>,  // Secondary UV coordinates
}

// Vertex shader output - matches C++ vertex shader
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) tex_coords0: vec2<f32>,
    @location(3) tex_coords1: vec2<f32>,
    @location(4) vertex_diffuse: vec4<f32>,
    @location(5) view_direction: vec3<f32>,
}

// C++ SAGE vertex shader - matches D3D8 vertex shader behavior
@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // Transform vertex to world space (identity transform for now)
    var world_position = vec4<f32>(input.position, 1.0);
    out.world_position = world_position.xyz;

    // Transform normal to world space
    out.world_normal = normalize(input.normal);

    // Pass through texture coordinates
    out.tex_coords0 = input.tex_coords0;
    out.tex_coords1 = input.tex_coords1;

    // Pass through vertex diffuse color
    out.vertex_diffuse = input.diffuse;

    // Calculate view direction for specular lighting
    out.view_direction = normalize(sage_uniforms.camera_position.xyz - out.world_position);

    // Transform to clip space
    out.clip_position = sage_uniforms.view_projection * world_position;

    return out;
}

// C++ SAGE pixel shader - matches D3D8 pixel shader with C&C lighting model
@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Sample primary texture (stage 0) - matches C++ texture sampling
    var stage0_color = textureSample(stage0_texture, stage0_sampler, input.tex_coords0);

    // Apply material diffuse color to texture - matches C++ VertexMaterialClass behavior
    // In D3D8, materials multiply textures with diffuse color and vertex color
    var tinted_texture = stage0_color * vec4<f32>(material_properties.diffuse_color.rgb, 1.0);

    // Material base color combination - vertex diffuse further modulates the result
    var base_color = tinted_texture * input.vertex_diffuse;

    // C++ SAGE lighting calculations
    var normal = normalize(input.world_normal);
    var light_dir = normalize(sage_uniforms.sun_direction);
    var view_dir = normalize(input.view_direction);

    // Ambient lighting (always present in C&C)
    var ambient = sage_uniforms.ambient_light;

    // Diffuse lighting (Lambertian) - core C&C lighting
    var diffuse_factor = max(dot(normal, -light_dir), 0.0);
    var diffuse = sage_uniforms.sun_color * diffuse_factor;

    // Specular lighting (Phong) - for shiny surfaces like vehicles
    var reflect_dir = reflect(light_dir, normal);
    var specular_factor = pow(max(dot(view_dir, reflect_dir), 0.0), 32.0); // Default shininess
    var specular = sage_uniforms.sun_color * specular_factor * 0.3; // Moderate specular

    // Final lighting combination - matches C++ SAGE lighting model
    var lighting = ambient + diffuse + specular;
    var final_color = vec4<f32>(base_color.rgb * lighting, base_color.a);

    // Ensure minimum visibility (C&C never goes completely black)
    final_color.r = max(final_color.r, 0.1);
    final_color.g = max(final_color.g, 0.1);
    final_color.b = max(final_color.b, 0.1);

    return final_color;
}
"#
    }
}
