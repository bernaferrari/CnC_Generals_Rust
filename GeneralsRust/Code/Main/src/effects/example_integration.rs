use game_engine::common::frame_clock::FrameClock;
use glam::{Mat4, Vec3};
use log::{error, info};
use std::sync::Arc;
use std::time::Duration;
use ww3d_engine::FrameTiming;

use crate::assets::archive::ArchiveFileSystem;
use crate::effects::audio_integration::Faction;
use crate::effects::{EffectsIntegration, QualityLevel};
use crate::game_logic::ObjectId;

/// Example game state with integrated effects
pub struct GameWithEffects {
    effects: EffectsIntegration,
    archive_system: ArchiveFileSystem,
    camera_position: Vec3,
    camera_forward: Vec3,
    camera_up: Vec3,
    is_initialized: bool,
    frame_clock: FrameClock,
}

impl GameWithEffects {
    /// Initialize the game with full effects integration
    pub async fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        archive_paths: Vec<String>,
        quality_level: QualityLevel,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        info!("Initializing C&C Generals with complete effects system");

        // Initialize archive system
        let mut archive_system = ArchiveFileSystem::new();
        for path in archive_paths {
            if let Err(e) = archive_system.mount_archive(&path).await {
                error!("Failed to mount archive {}: {}", path, e);
            }
        }

        // Initialize effects integration
        let mut effects = EffectsIntegration::new(device, queue, quality_level).await?;
        effects.initialize(&mut archive_system).await?;

        Ok(Self {
            effects,
            archive_system,
            camera_position: Vec3::ZERO,
            camera_forward: Vec3::NEG_Z,
            camera_up: Vec3::Y,
            is_initialized: true,
            frame_clock: FrameClock::new(),
        })
    }

    /// Main game update loop
    pub fn update(&mut self, delta_time: f32) {
        if !self.is_initialized {
            return;
        }

        let delta = Duration::from_secs_f32(delta_time.max(0.0));
        let timing = self.frame_clock.advance_fixed(delta);
        self.update_with_timing(&timing);
    }

    /// Update loop that consumes canonical WW3D timing data
    pub fn update_with_timing(&mut self, timing: &FrameTiming) {
        if !self.is_initialized {
            return;
        }

        self.effects.update_with_timing(
            timing,
            self.camera_position,
            self.camera_forward,
            self.camera_up,
        );

        let screen_shake = self.effects.get_screen_shake();
        if screen_shake.length() > 0.001 {
            self.camera_position += screen_shake * 0.1;
        }
    }

    /// Render the game with effects
    pub fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
    ) {
        if !self.is_initialized {
            return;
        }

        // Render all effects
        self.effects.render(encoder, view, depth_view);

        // Get camera flash for post-processing
        let (flash_color, flash_intensity) = self.effects.get_camera_flash();
        if flash_intensity > 0.01 {
            // Apply screen flash effect (would be handled by post-processing pipeline)
            info!(
                "Camera flash: color={:?}, intensity={:.2}",
                flash_color, flash_intensity
            );
        }
    }

    /// Set camera position and orientation
    pub fn set_camera(&mut self, position: Vec3, forward: Vec3, up: Vec3) {
        self.camera_position = position;
        self.camera_forward = forward;
        self.camera_up = up;
    }

    /// Example: Tank destroys building
    pub async fn example_tank_destroys_building(
        &mut self,
        tank_id: ObjectId,
        tank_pos: Vec3,
        building_id: ObjectId,
        building_pos: Vec3,
        building_size: f32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!("Tank destroys building sequence");

        // 1. Tank fires at building
        self.effects
            .create_weapon_fire(
                "tank_cannon",
                tank_pos + Vec3::new(2.0, 0.0, 1.0), // Muzzle offset
                Some(building_pos),
                Some(tank_id),
                &mut self.archive_system,
            )
            .await?;

        // 2. Tank unit acknowledgment
        self.effects
            .play_unit_voice(
                "M1A1Tank",
                "attack",
                Faction::USA,
                Some(tank_pos),
                &mut self.archive_system,
            )
            .await?;

        // 3. Building takes damage and explodes
        tokio::time::sleep(tokio::time::Duration::from_millis(800)).await;

        self.effects
            .create_explosion(
                building_pos,
                building_size,
                "building",
                &mut self.archive_system,
            )
            .await?;

        // 4. Stop all building effects
        self.effects.stop_all_object_effects(building_id);

        Ok(())
    }

    /// Example: Unit construction sequence
    pub async fn example_unit_construction(
        &mut self,
        factory_id: ObjectId,
        factory_pos: Vec3,
        unit_type: &str,
        faction: Faction,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!("Unit construction sequence: {}", unit_type);

        // 1. Factory construction animation and effects
        self.effects
            .building_constructed(
                factory_pos,
                "WarFactory",
                factory_id,
                &mut self.archive_system,
            )
            .await?;

        // 2. Unit creation sound
        tokio::time::sleep(tokio::time::Duration::from_millis(3000)).await;

        self.effects
            .play_unit_voice(
                unit_type,
                "create",
                faction,
                Some(factory_pos),
                &mut self.archive_system,
            )
            .await?;

        Ok(())
    }

    /// Example: Battle scene with multiple effects
    pub async fn example_battle_scene(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Starting battle scene example");

        // Set up battlefield lighting (dawn lighting)
        self.effects.set_ambient_lighting(
            Vec3::new(0.6, 0.5, 0.4), // Warm dawn colors
            0.4,
        );

        // Start battle music
        self.effects
            .play_faction_music(Faction::USA, &mut self.archive_system)
            .await?;

        // Simulate multiple simultaneous effects
        let battles = vec![
            (Vec3::new(10.0, 0.0, 5.0), Vec3::new(15.0, 0.0, 8.0)),
            (Vec3::new(-8.0, 0.0, 3.0), Vec3::new(-12.0, 0.0, 7.0)),
            (Vec3::new(0.0, 0.0, -10.0), Vec3::new(5.0, 0.0, -15.0)),
        ];

        for (i, (pos1, pos2)) in battles.iter().enumerate() {
            let tank_id = ObjectId::new(100 + i as u32);
            let target_id = ObjectId::new(200 + i as u32);

            // Stagger the attacks
            let delay = i as u64 * 500;
            tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;

            // Tank fires
            self.effects
                .create_weapon_fire(
                    "tank_cannon",
                    *pos1,
                    Some(*pos2),
                    Some(tank_id),
                    &mut self.archive_system,
                )
                .await?;

            // Impact after flight time
            tokio::time::sleep(tokio::time::Duration::from_millis(600)).await;

            self.effects
                .create_explosion(*pos2, 1.2, "tank", &mut self.archive_system)
                .await?;

            // Unit movement with dust
            self.effects.unit_moving(tank_id, *pos1, "tank").await?;
        }

        info!("Battle scene complete");
        Ok(())
    }

    /// Example: Environmental effects showcase
    pub async fn example_environmental_effects(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!("Environmental effects showcase");

        // Various environmental effects
        let effects = vec![
            ("dust_trail", Vec3::new(5.0, 0.0, 0.0)),
            ("smoke_trail", Vec3::new(-5.0, 0.0, 0.0)),
            ("construction_sparks", Vec3::new(0.0, 0.0, 5.0)),
            ("engine_exhaust", Vec3::new(0.0, 0.0, -5.0)),
        ];

        for (effect_type, position) in effects {
            self.effects
                .create_environmental_effect(effect_type, position, 3.0, None)
                .await?;

            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }

        Ok(())
    }

    /// Example: Quality level adjustment based on performance
    pub fn example_quality_adjustment(&mut self) {
        let stats = self.effects.get_performance_stats();
        let fps = self.effects.get_current_fps();

        info!(
            "Performance stats: {:.1} FPS, Particles: {}/{}, Lights: {}/{}, Effects: {}/{}",
            fps,
            stats.particles_used,
            stats.particles_budget,
            stats.lights_used,
            stats.lights_budget,
            stats.effects_used,
            stats.effects_budget
        );

        // Manual quality adjustment example
        if fps < 30.0 {
            info!("Low FPS detected, reducing quality");
            self.effects.set_quality_level(QualityLevel::Low);
        } else if fps > 50.0 && stats.quality_level == QualityLevel::Low {
            info!("Good FPS, increasing quality");
            self.effects.set_quality_level(QualityLevel::Medium);
        }
    }

    /// Example: Audio system showcase
    pub async fn example_audio_showcase(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Audio system showcase");

        // Different faction voices
        let factions = vec![
            (Faction::USA, "Ranger ready!"),
            (Faction::China, "Red Guard standing by!"),
            (Faction::GLA, "Ready for action!"),
        ];

        for (faction, _description) in factions {
            // Unit selection
            self.effects
                .play_unit_voice(
                    "Infantry",
                    "select",
                    faction,
                    Some(Vec3::new(fastrand::f32() * 10.0 - 5.0, 0.0, 0.0)),
                    &mut self.archive_system,
                )
                .await?;

            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

            // Unit movement
            self.effects
                .play_unit_voice(
                    "Infantry",
                    "move",
                    faction,
                    Some(Vec3::new(fastrand::f32() * 10.0 - 5.0, 0.0, 0.0)),
                    &mut self.archive_system,
                )
                .await?;

            tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;
        }

        Ok(())
    }

    /// Get current effects system reference
    pub fn get_effects(&self) -> &EffectsIntegration {
        &self.effects
    }

    /// Get mutable effects system reference
    pub fn get_effects_mut(&mut self) -> &mut EffectsIntegration {
        &mut self.effects
    }

    /// Set master volumes
    pub fn set_audio_volumes(&mut self, master: f32, music: f32, sfx: f32, voice: f32) {
        self.effects.set_master_volume(master);
        self.effects.set_music_volume(music);
        self.effects.set_sfx_volume(sfx);
        self.effects.set_voice_volume(voice);

        info!(
            "Audio volumes: Master={:.1}%, Music={:.1}%, SFX={:.1}%, Voice={:.1}%",
            master * 100.0,
            music * 100.0,
            sfx * 100.0,
            voice * 100.0
        );
    }
}

/// Example of integrating effects into existing game loop
pub async fn integration_example() -> Result<(), Box<dyn std::error::Error>> {
    use wgpu::Instance;

    info!("C&C Generals Effects Integration Example");

    // Mock WGPU setup (in real game, this comes from renderer)
    let instance = Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions::default())
        .await?;

    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor::default())
        .await?;

    let device = Arc::new(device);
    let queue = Arc::new(queue);

    // Initialize game with effects
    let mut game = GameWithEffects::new(
        device,
        queue,
        vec![
            "assets/AudioZH.big".to_string(),
            "assets/MusicZH.big".to_string(),
            "assets/SpeechEnglishZH.big".to_string(),
        ],
        QualityLevel::High,
    )
    .await?;

    // Set up scene
    game.set_camera(
        Vec3::new(0.0, 10.0, 20.0),             // Position
        Vec3::new(0.0, -0.5, -1.0).normalize(), // Forward
        Vec3::Y,                                // Up
    );

    // Set audio volumes
    game.set_audio_volumes(0.8, 0.7, 0.9, 1.0);

    // Enable auto quality adjustment
    game.get_effects_mut().set_auto_quality_adjustment(true);

    // Run example scenarios
    info!("Running battle scene...");
    game.example_battle_scene().await?;

    info!("Running environmental effects...");
    game.example_environmental_effects().await?;

    info!("Running audio showcase...");
    game.example_audio_showcase().await?;

    // Simulate game loop
    info!("Simulating game loop...");
    for i in 0..60 {
        let delta_time = 1.0 / 30.0; // C++ logic rate (30 FPS)
        game.update(delta_time);

        if i % 10 == 0 {
            game.example_quality_adjustment();
        }

        // In real game, this would be the actual render call
        // game.render(&mut encoder, &view, &depth_view);

        tokio::time::sleep(tokio::time::Duration::from_millis(16)).await;
    }

    info!("Effects integration example completed successfully!");
    Ok(())
}
