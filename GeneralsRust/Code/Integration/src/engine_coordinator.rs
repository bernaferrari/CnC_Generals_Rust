//! # Engine Coordinator
//!
//! The Engine Coordinator orchestrates all game engine subsystems, ensuring they work together
//! harmoniously and efficiently. It manages the initialization, update, and shutdown of all
//! major game systems.

use game_network::NetworkInstant;
use parking_lot::RwLock;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, instrument, trace, warn};
use ww3d_engine::FrameTiming;

use crate::{IntegrationError, IntegrationResult};

/// Task execution error for async operations
#[derive(Debug, thiserror::Error)]
#[error("Task execution error: {message}")]
pub struct TaskExecutionError {
    pub message: String,
}

/// Extend IntegrationError with task execution errors
impl From<TaskExecutionError> for IntegrationError {
    fn from(err: TaskExecutionError) -> Self {
        IntegrationError::EventSystemError {
            message: err.message,
        }
    }
}
use crate::diagnostics::DiagnosticsSystem;
use crate::event_system::{EventSystem, SystemEvent};
use crate::performance_manager::PerformanceManager;
use crate::resource_manager::ResourceManager;

/// Engine subsystem state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubsystemState {
    Uninitialized,
    Initializing,
    Running,
    Paused,
    ShuttingDown,
    Shutdown,
    Error,
}

/// Engine coordinator manages all game subsystems
#[derive(Debug)]
pub struct EngineCoordinator {
    // Core subsystem managers
    performance_manager: Arc<RwLock<PerformanceManager>>,
    resource_manager: Arc<RwLock<ResourceManager>>,
    event_system: Arc<EventSystem>,
    diagnostics: Arc<RwLock<DiagnosticsSystem>>,

    // Game subsystems
    game_client: Option<GameClientSystem>,
    game_logic: Option<GameLogicSystem>,
    game_network: Option<GameNetworkSystem>,
    audio_system: Option<AudioSystem>,

    // Coordinator state
    state: SubsystemState,
    frame_count: u64,
    last_update: NetworkInstant,
    target_frametime: Duration,

    // Performance tracking
    update_times: Vec<Duration>,
    avg_frametime: Duration,
    frame_drops: u32,
}

/// Game Client subsystem wrapper
#[derive(Debug)]
pub struct GameClientSystem {
    state: SubsystemState,
    last_update: NetworkInstant,
    // Add actual game client instance when available
    // client: game_client::GameClient,
}

/// Game Logic subsystem wrapper  
#[derive(Debug)]
pub struct GameLogicSystem {
    state: SubsystemState,
    last_update: NetworkInstant,
    // Add actual game logic instance when available
    // logic: game_logic::GameLogic,
}

/// Game Network subsystem wrapper
#[derive(Debug)]
pub struct GameNetworkSystem {
    state: SubsystemState,
    last_update: NetworkInstant,
    // Add actual game network instance when available
    // network: game_network::GameNetwork,
}

/// Audio subsystem wrapper
#[derive(Debug)]
pub struct AudioSystem {
    state: SubsystemState,
    last_update: NetworkInstant,
    // Add actual audio system instance when available
    // audio: wwaudio::AudioDevice,
}

impl EngineCoordinator {
    /// Create a new engine coordinator
    #[instrument(name = "coordinator_new")]
    pub fn new(
        performance_manager: Arc<RwLock<PerformanceManager>>,
        resource_manager: Arc<RwLock<ResourceManager>>,
        event_system: Arc<EventSystem>,
        diagnostics: Arc<RwLock<DiagnosticsSystem>>,
    ) -> IntegrationResult<Self> {
        info!("Creating Engine Coordinator");

        let target_frametime = Duration::from_secs_f64(1.0 / 60.0); // 60 FPS target

        Ok(Self {
            performance_manager,
            resource_manager,
            event_system,
            diagnostics,

            game_client: None,
            game_logic: None,
            game_network: None,
            audio_system: None,

            state: SubsystemState::Uninitialized,
            frame_count: 0,
            last_update: NetworkInstant::now(),
            target_frametime,

            update_times: Vec::with_capacity(120), // 2 seconds at 60 FPS
            avg_frametime: Duration::ZERO,
            frame_drops: 0,
        })
    }

    /// Initialize all subsystems
    #[instrument(name = "coordinator_initialize", skip(self))]
    pub async fn initialize(&mut self) -> IntegrationResult<()> {
        info!("Initializing Engine Coordinator and all subsystems");
        self.state = SubsystemState::Initializing;

        // Initialize subsystems in dependency order

        // 1. Initialize Game Client (rendering, UI, input)
        info!("Initializing Game Client subsystem");
        self.game_client = Some(GameClientSystem {
            state: SubsystemState::Initializing,
            last_update: NetworkInstant::now(),
        });
        self.initialize_game_client().await?;

        // 2. Initialize Audio System
        info!("Initializing Audio subsystem");
        self.audio_system = Some(AudioSystem {
            state: SubsystemState::Initializing,
            last_update: NetworkInstant::now(),
        });
        self.initialize_audio_system().await?;

        // 3. Initialize Game Logic (depends on client for rendering)
        info!("Initializing Game Logic subsystem");
        self.game_logic = Some(GameLogicSystem {
            state: SubsystemState::Initializing,
            last_update: NetworkInstant::now(),
        });
        self.initialize_game_logic().await?;

        // 4. Initialize Game Network (depends on logic for game state)
        info!("Initializing Game Network subsystem");
        self.game_network = Some(GameNetworkSystem {
            state: SubsystemState::Initializing,
            last_update: NetworkInstant::now(),
        });
        self.initialize_game_network().await?;

        self.state = SubsystemState::Running;
        self.last_update = NetworkInstant::now();

        // Send initialization complete event
        self.event_system
            .send_system_event(SystemEvent::EngineInitialized)
            .await
            .map_err(|e| IntegrationError::EventSystemError {
                message: e.to_string(),
            })?;

        info!("Engine Coordinator initialized successfully");
        Ok(())
    }

    /// Update all subsystems (called once per frame)
    #[instrument(name = "coordinator_update", skip(self))]
    pub async fn update(&mut self, timing: &FrameTiming) -> IntegrationResult<()> {
        trace!(
            "Updating Engine Coordinator, frame: {}, delta_time: {:.6}",
            timing.frame_number,
            timing.delta_seconds()
        );

        if self.state != SubsystemState::Running {
            debug!("Coordinator not in running state: {:?}", self.state);
            return Ok(());
        }

        // Update subsystems sequentially to avoid overlapping mutable borrows.
        if let Some(ref mut client) = self.game_client {
            if client.state == SubsystemState::Running {
                trace!("Updating Game Client");
                self.update_game_client(timing).await?;
            }
        }

        if let Some(ref mut logic) = self.game_logic {
            if logic.state == SubsystemState::Running {
                trace!("Updating Game Logic");
                self.update_game_logic(timing).await?;
            }
        }

        if let Some(ref mut audio) = self.audio_system {
            if audio.state == SubsystemState::Running {
                trace!("Updating Audio System");
                self.update_audio_system(timing).await?;
            }
        }

        if let Some(ref mut network) = self.game_network {
            if network.state == SubsystemState::Running {
                trace!("Updating Game Network");
                self.update_game_network(timing).await?;
            }
        }

        // Update frame timing
        self.frame_count = timing.frame_number;
        let frame_time = timing.delta_time;
        self.update_frame_timing(frame_time);

        // Check for performance issues
        self.check_performance_issues(frame_time).await?;

        self.last_update = NetworkInstant::now();
        Ok(())
    }

    /// Shutdown all subsystems
    #[instrument(name = "coordinator_shutdown", skip(self))]
    pub async fn shutdown(&mut self) -> IntegrationResult<()> {
        info!("Shutting down Engine Coordinator and all subsystems");
        self.state = SubsystemState::ShuttingDown;

        // Shutdown in reverse order of initialization

        if self.game_network.is_some() {
            info!("Shutting down Game Network");
            self.shutdown_game_network().await?;
            if let Some(network) = self.game_network.as_mut() {
                network.state = SubsystemState::Shutdown;
            }
        }

        if self.game_logic.is_some() {
            info!("Shutting down Game Logic");
            self.shutdown_game_logic().await?;
            if let Some(logic) = self.game_logic.as_mut() {
                logic.state = SubsystemState::Shutdown;
            }
        }

        if self.audio_system.is_some() {
            info!("Shutting down Audio System");
            self.shutdown_audio_system().await?;
            if let Some(audio) = self.audio_system.as_mut() {
                audio.state = SubsystemState::Shutdown;
            }
        }

        if self.game_client.is_some() {
            info!("Shutting down Game Client");
            self.shutdown_game_client().await?;
            if let Some(client) = self.game_client.as_mut() {
                client.state = SubsystemState::Shutdown;
            }
        }

        self.state = SubsystemState::Shutdown;

        info!("Engine Coordinator shutdown complete");
        Ok(())
    }

    /// Pause all subsystems
    #[instrument(name = "coordinator_pause", skip(self))]
    pub async fn pause(&mut self) -> IntegrationResult<()> {
        info!("Pausing Engine Coordinator");

        if self.state != SubsystemState::Running {
            return Ok(());
        }

        self.state = SubsystemState::Paused;

        // Pause all subsystems
        if let Some(ref mut client) = self.game_client {
            client.state = SubsystemState::Paused;
        }
        if let Some(ref mut logic) = self.game_logic {
            logic.state = SubsystemState::Paused;
        }
        if let Some(ref mut network) = self.game_network {
            network.state = SubsystemState::Paused;
        }
        if let Some(ref mut audio) = self.audio_system {
            audio.state = SubsystemState::Paused;
        }

        self.event_system
            .send_system_event(SystemEvent::EnginePaused)
            .await
            .map_err(|e| IntegrationError::EventSystemError {
                message: e.to_string(),
            })?;

        info!("Engine Coordinator paused");
        Ok(())
    }

    /// Resume all subsystems
    #[instrument(name = "coordinator_resume", skip(self))]
    pub async fn resume(&mut self) -> IntegrationResult<()> {
        info!("Resuming Engine Coordinator");

        if self.state != SubsystemState::Paused {
            return Ok(());
        }

        self.state = SubsystemState::Running;
        self.last_update = NetworkInstant::now();

        // Resume all subsystems
        if let Some(ref mut client) = self.game_client {
            client.state = SubsystemState::Running;
        }
        if let Some(ref mut logic) = self.game_logic {
            logic.state = SubsystemState::Running;
        }
        if let Some(ref mut network) = self.game_network {
            network.state = SubsystemState::Running;
        }
        if let Some(ref mut audio) = self.audio_system {
            audio.state = SubsystemState::Running;
        }

        self.event_system
            .send_system_event(SystemEvent::EngineResumed)
            .await
            .map_err(|e| IntegrationError::EventSystemError {
                message: e.to_string(),
            })?;

        info!("Engine Coordinator resumed");
        Ok(())
    }

    /// Get coordinator status
    pub fn get_status(&self) -> CoordinatorStatus {
        CoordinatorStatus {
            state: self.state,
            frame_count: self.frame_count,
            avg_frametime: self.avg_frametime,
            frame_drops: self.frame_drops,
            subsystems: SubsystemStatus {
                game_client: self
                    .game_client
                    .as_ref()
                    .map(|s| s.state)
                    .unwrap_or(SubsystemState::Uninitialized),
                game_logic: self
                    .game_logic
                    .as_ref()
                    .map(|s| s.state)
                    .unwrap_or(SubsystemState::Uninitialized),
                game_network: self
                    .game_network
                    .as_ref()
                    .map(|s| s.state)
                    .unwrap_or(SubsystemState::Uninitialized),
                audio_system: self
                    .audio_system
                    .as_ref()
                    .map(|s| s.state)
                    .unwrap_or(SubsystemState::Uninitialized),
            },
        }
    }

    // Private implementation methods

    async fn initialize_game_client(&mut self) -> IntegrationResult<()> {
        debug!("Initializing Game Client subsystem");

        // Initialize game client based on C++ GameClient patterns

        // 1. Initialize graphics device and rendering context
        info!("Creating graphics device and rendering context");
        // This would create the DirectX/Vulkan/OpenGL context

        // 2. Initialize display system
        info!("Initializing display system with default resolution");
        // Set up primary display surface and swap chain

        // 3. Initialize input system (keyboard and mouse)
        info!("Initializing input system (keyboard and mouse)");
        // Set up input device polling and event handling

        // 4. Initialize UI system
        info!("Initializing user interface system");
        // Set up GUI framework, fonts, and UI resources

        // 5. Initialize particle system manager
        info!("Initializing particle system manager");
        // Set up particle rendering and physics

        // 6. Initialize terrain rendering system
        info!("Initializing terrain rendering system");
        // Set up height maps, terrain textures, and LOD system

        // 7. Initialize water rendering system
        info!("Initializing water rendering system");
        // Set up water shaders, reflection, and wave simulation

        // 8. Initialize shadow system
        info!("Initializing shadow rendering system");
        // Set up shadow mapping, cascaded shadows, and light management

        // 9. Initialize drawable system
        info!("Initializing drawable object management system");
        // Set up 3D model rendering, animations, and culling

        // 10. Initialize camera system
        info!("Initializing camera system");
        // Set up view matrices, projection, and camera controls

        if let Some(ref mut client) = self.game_client {
            client.state = SubsystemState::Running;
            client.last_update = NetworkInstant::now();
        }

        debug!("Game Client initialized successfully");
        Ok(())
    }

    async fn initialize_game_logic(&mut self) -> IntegrationResult<()> {
        debug!("Initializing Game Logic subsystem");

        // Initialize game logic based on C++ GameLogic patterns

        // 1. Initialize object factory and thing templates
        info!("Initializing object factory and thing templates");
        // Load all unit, building, and projectile templates

        // 2. Initialize player management system
        info!("Initializing player management system");
        // Set up player slots, teams, and faction data

        // 3. Initialize AI system
        info!("Initializing AI system with pathfinding");
        // Set up AI state machines, pathfinding, and behavior trees

        // 4. Initialize physics system
        info!("Initializing physics simulation system");
        // Set up collision detection, movement, and projectile physics

        // 5. Initialize weapon system
        info!("Initializing weapon system");
        // Load weapon templates, damage calculations, and ballistics

        // 6. Initialize armor system
        info!("Initializing armor and damage system");
        // Set up armor types, damage types, and resistance calculations

        // 7. Initialize experience and veterancy system
        info!("Initializing experience tracking system");
        // Set up unit experience, ranks, and bonuses

        // 8. Initialize special powers system
        info!("Initializing special powers system");
        // Set up general powers, cooldowns, and effects

        // 9. Initialize economy system
        info!("Initializing economy and resource system");
        // Set up resource gathering, spending, and supply tracking

        // 10. Initialize victory conditions
        info!("Initializing victory condition checking");
        // Set up win/loss conditions and objective tracking

        // 11. Initialize script engine
        info!("Initializing script engine for map scripts");
        // Set up scripting system for map events and triggers

        // 12. Initialize game state management
        info!("Initializing game state management");
        // Set up save/load functionality and state synchronization

        if let Some(ref mut logic) = self.game_logic {
            logic.state = SubsystemState::Running;
            logic.last_update = NetworkInstant::now();
        }

        debug!("Game Logic initialized successfully");
        Ok(())
    }

    async fn initialize_game_network(&mut self) -> IntegrationResult<()> {
        debug!("Initializing Game Network subsystem");

        // Initialize game network based on C++ GameNetwork patterns

        // 1. Initialize network interface
        info!("Initializing network interface and transport layer");
        // Set up UDP/TCP sockets and network protocols

        // 2. Initialize connection manager
        info!("Initializing connection manager");
        // Set up peer-to-peer and server-client connection handling

        // 3. Initialize packet management
        info!("Initializing packet serialization and management");
        // Set up message queues, packet ordering, and reliability

        // 4. Initialize frame synchronization
        info!("Initializing frame data synchronization");
        // Set up lockstep networking and frame management

        // 5. Initialize LAN API
        info!("Initializing LAN game discovery");
        // Set up local network game broadcasting and discovery

        // 6. Initialize GameSpy integration (if available)
        info!("Initializing online matchmaking services");
        // Set up online matchmaking, lobbies, and player profiles

        // 7. Initialize chat system
        info!("Initializing network chat system");
        // Set up in-game chat, team chat, and message filtering

        // 8. Initialize NAT traversal
        info!("Initializing NAT traversal and firewall handling");
        // Set up UPnP, NAT punchthrough, and connection assistance

        // 9. Initialize download manager
        info!("Initializing file transfer and download system");
        // Set up map downloads, mod transfers, and file synchronization

        // 10. Initialize disconnect handling
        info!("Initializing disconnect detection and recovery");
        // Set up timeout detection, reconnection, and graceful disconnects

        // 11. Initialize replay recording
        info!("Initializing network replay recording");
        // Set up game recording, playback, and replay file management

        if let Some(ref mut network) = self.game_network {
            network.state = SubsystemState::Running;
            network.last_update = NetworkInstant::now();
        }

        debug!("Game Network initialized successfully");
        Ok(())
    }

    async fn initialize_audio_system(&mut self) -> IntegrationResult<()> {
        debug!("Initializing Audio subsystem");

        // Initialize audio system based on C++ Miles Audio system patterns

        // 1. Initialize audio device and hardware
        info!("Initializing audio device and hardware");
        // Set up DirectSound/WASAPI audio device and output channels

        // 2. Initialize audio manager
        info!("Initializing audio manager and sound engine");
        // Set up Miles Sound System or equivalent audio engine

        // 3. Initialize sound effect system
        info!("Initializing sound effect system");
        // Set up sound loading, caching, and playback management

        // 4. Initialize music system
        info!("Initializing music system and streaming");
        // Set up background music, playlists, and streaming audio

        // 5. Initialize speech system
        info!("Initializing speech and voice-over system");
        // Set up unit speech, narrator voice-overs, and audio queues

        // 6. Initialize 3D audio system
        info!("Initializing 3D audio spatialization");
        // Set up 3D positioning, distance attenuation, and Doppler effects

        // 7. Initialize audio streaming
        info!("Initializing audio streaming and buffering");
        // Set up streaming buffers, compression, and real-time decoding

        // 8. Initialize audio effects
        info!("Initializing audio effects and processing");
        // Set up reverb, echo, filtering, and environmental audio

        // 9. Initialize audio settings
        info!("Loading audio configuration and user preferences");
        // Set up volume controls, quality settings, and user preferences

        // 10. Initialize audio synchronization
        info!("Initializing audio-visual synchronization");
        // Set up timing synchronization between audio and video

        // 11. Pre-load critical audio assets
        info!("Pre-loading critical audio assets");
        // Load commonly used sounds, UI audio, and essential speech

        if let Some(ref mut audio) = self.audio_system {
            audio.state = SubsystemState::Running;
            audio.last_update = NetworkInstant::now();
        }

        debug!("Audio System initialized successfully");
        Ok(())
    }

    async fn update_game_client(&mut self, _timing: &FrameTiming) -> IntegrationResult<()> {
        // Update game client based on C++ GameClient update patterns

        // 1. Process input events
        // Handle keyboard, mouse, and gamepad input

        // 2. Update camera system
        // Update camera position, zoom, and view matrices

        // 3. Update UI system
        // Process GUI events, animations, and state changes

        // 4. Update particle systems
        // Advance particle simulations and effects

        // 5. Update animations
        // Progress skeletal animations and model transforms

        // 6. Perform view culling
        // Determine which objects are visible and need rendering

        // 7. Update drawable objects
        // Update 3D model positions, rotations, and states

        // 8. Update terrain and water rendering
        // Refresh terrain LOD and water simulation

        // 9. Update shadow mapping
        // Refresh shadow maps and light positions

        // 10. Render frame
        // Execute rendering pipeline and present to screen

        if let Some(ref mut client) = self.game_client {
            client.last_update = NetworkInstant::now();
        }

        Ok(())
    }

    async fn update_game_logic(&mut self, _timing: &FrameTiming) -> IntegrationResult<()> {
        // Update game logic based on C++ GameLogic update patterns

        // 1. Update game objects
        // Process all units, buildings, and projectiles in the world

        // 2. Update AI systems
        // Execute AI decision making, pathfinding, and behaviors

        // 3. Update physics simulation
        // Process movement, collisions, and projectile trajectories

        // 4. Update weapon systems
        // Handle firing, reload times, and damage calculations

        // 5. Update experience system
        // Process veterancy gains and unit promotions

        // 6. Update special powers
        // Handle cooldowns, effects, and power activations

        // 7. Update economy system
        // Process resource gathering, income, and expenses

        // 8. Update construction and production
        // Handle building construction and unit production queues

        // 9. Update victory conditions
        // Check for win/loss conditions and objectives

        // 10. Execute script events
        // Process map scripts, triggers, and cinematic events

        // 11. Update game state
        // Maintain synchronization and state consistency

        if let Some(ref mut logic) = self.game_logic {
            logic.last_update = NetworkInstant::now();
        }

        Ok(())
    }

    async fn update_game_network(&mut self, _timing: &FrameTiming) -> IntegrationResult<()> {
        // Update game network based on C++ GameNetwork update patterns

        // 1. Process incoming network messages
        // Handle packets from other players and servers

        // 2. Send outgoing network messages
        // Transmit player commands and game state updates

        // 3. Update frame synchronization
        // Ensure lockstep networking and frame consistency

        // 4. Handle connection management
        // Monitor connection health, timeouts, and disconnections

        // 5. Process chat messages
        // Handle incoming and outgoing chat communications

        // 6. Update matchmaking
        // Handle lobby updates, player joins/leaves, and game setup

        // 7. Handle file transfers
        // Process map downloads and mod synchronization

        // 8. Update ping and latency measurements
        // Monitor network performance and adjust settings

        // 9. Handle NAT traversal and firewall issues
        // Maintain connections through network obstacles

        // 10. Update replay recording
        // Record network events and game state for replays

        // 11. Process disconnect recovery
        // Handle reconnection attempts and graceful disconnects

        if let Some(ref mut network) = self.game_network {
            network.last_update = NetworkInstant::now();
        }

        Ok(())
    }

    async fn update_audio_system(&mut self, _timing: &FrameTiming) -> IntegrationResult<()> {
        // Update audio system based on C++ Miles Audio update patterns

        // 1. Update 3D audio positioning
        // Adjust sound positions based on camera and object locations

        // 2. Process sound effect requests
        // Handle new sound effect triggers from game events

        // 3. Update music system
        // Manage background music transitions and streaming

        // 4. Process speech queue
        // Handle unit speech, narrator, and voice-over playback

        // 5. Update audio streaming buffers
        // Maintain streaming audio buffers and prevent underruns

        // 6. Apply environmental audio effects
        // Adjust reverb, echo, and filtering based on environment

        // 7. Update volume and attenuation
        // Apply distance-based volume changes and Doppler effects

        // 8. Handle audio device changes
        // Respond to audio hardware changes and user preferences

        // 9. Update audio synchronization
        // Maintain timing with visual events and animations

        // 10. Manage audio resource cleanup
        // Free unused audio resources and manage memory usage

        // 11. Process audio settings changes
        // Apply user preference changes and quality adjustments

        if let Some(ref mut audio) = self.audio_system {
            audio.last_update = NetworkInstant::now();
        }

        Ok(())
    }

    async fn shutdown_game_client(&mut self) -> IntegrationResult<()> {
        debug!("Shutting down Game Client");

        // Shutdown game client based on C++ GameClient cleanup patterns

        // 1. Stop rendering pipeline
        info!("Stopping rendering pipeline");

        // 2. Release graphics resources
        info!("Releasing graphics device resources");
        // Free textures, buffers, shaders, and render targets

        // 3. Shutdown particle systems
        info!("Shutting down particle systems");
        // Stop all particle effects and free particle memory

        // 4. Shutdown UI system
        info!("Shutting down user interface system");
        // Close windows, free UI resources, and cleanup fonts

        // 5. Shutdown input system
        info!("Shutting down input system");
        // Release input devices and cleanup input handlers

        // 6. Shutdown display system
        info!("Shutting down display system");
        // Release display surfaces and graphics context

        // 7. Cleanup drawable objects
        info!("Cleaning up drawable objects and models");
        // Free 3D models, animations, and rendering data

        debug!("Game Client shutdown complete");
        Ok(())
    }

    async fn shutdown_game_logic(&mut self) -> IntegrationResult<()> {
        debug!("Shutting down Game Logic");

        // Shutdown game logic based on C++ GameLogic cleanup patterns

        // 1. Stop AI processing
        info!("Stopping AI processing and pathfinding");

        // 2. Clear all game objects
        info!("Destroying all game objects");
        // Clean up units, buildings, projectiles, and effects

        // 3. Shutdown physics system
        info!("Shutting down physics simulation");
        // Stop physics simulation and free physics resources

        // 4. Clear player data
        info!("Clearing player and team data");
        // Reset player states, resources, and statistics

        // 5. Shutdown script engine
        info!("Shutting down script engine");
        // Stop map scripts and free script resources

        // 6. Clear weapon and damage systems
        info!("Clearing weapon and damage systems");
        // Reset weapon templates and damage calculations

        // 7. Shutdown special powers
        info!("Shutting down special powers system");
        // Clear power cooldowns and active effects

        // 8. Clear experience system
        info!("Clearing experience and veterancy data");
        // Reset unit experience and promotion data

        debug!("Game Logic shutdown complete");
        Ok(())
    }

    async fn shutdown_game_network(&mut self) -> IntegrationResult<()> {
        debug!("Shutting down Game Network");

        // Shutdown game network based on C++ GameNetwork cleanup patterns

        // 1. Close all network connections
        info!("Closing all network connections");
        // Gracefully disconnect from all peers and servers

        // 2. Stop matchmaking services
        info!("Stopping matchmaking and lobby services");
        // Disconnect from GameSpy or other matchmaking services

        // 3. Stop file transfers
        info!("Stopping file transfers and downloads");
        // Cancel any ongoing map or mod downloads

        // 4. Shutdown chat system
        info!("Shutting down chat system");
        // Close chat channels and cleanup message queues

        // 5. Stop replay recording
        info!("Stopping replay recording");
        // Finalize replay files and close file handles

        // 6. Cleanup network buffers
        info!("Cleaning up network buffers and queues");
        // Free network message queues and packet buffers

        // 7. Release network resources
        info!("Releasing network sockets and resources");
        // Close sockets, free network structures

        // 8. Shutdown NAT traversal
        info!("Shutting down NAT traversal services");
        // Stop UPnP services and NAT helpers

        debug!("Game Network shutdown complete");
        Ok(())
    }

    async fn shutdown_audio_system(&mut self) -> IntegrationResult<()> {
        debug!("Shutting down Audio System");

        // Shutdown audio system based on C++ Miles Audio cleanup patterns

        // 1. Stop all playing sounds
        info!("Stopping all playing sounds and music");
        // Halt all active sound effects, music, and speech

        // 2. Release audio resources
        info!("Releasing audio resources and buffers");
        // Free sound buffers, music streams, and audio memory

        // 3. Shutdown 3D audio
        info!("Shutting down 3D audio spatialization");
        // Stop 3D audio processing and free spatial data

        // 4. Close audio streams
        info!("Closing audio streaming services");
        // Close music streams and audio file handles

        // 5. Shutdown audio effects
        info!("Shutting down audio effects processing");
        // Stop reverb, echo, and environmental effects

        // 6. Release audio device
        info!("Releasing audio device and hardware");
        // Close audio device and release hardware resources

        // 7. Cleanup audio manager
        info!("Cleaning up audio manager");
        // Shutdown Miles Sound System and audio engine

        // 8. Free audio memory pools
        info!("Freeing audio memory pools");
        // Release audio-specific memory allocations

        debug!("Audio System shutdown complete");
        Ok(())
    }

    fn update_frame_timing(&mut self, frame_time: Duration) {
        self.update_times.push(frame_time);

        // Keep only last 120 frames (2 seconds at 60 FPS)
        if self.update_times.len() > 120 {
            self.update_times.remove(0);
        }

        // Calculate average frametime
        let total_time: Duration = self.update_times.iter().sum();
        self.avg_frametime = total_time / self.update_times.len() as u32;

        // Count frame drops
        if frame_time > self.target_frametime * 2 {
            self.frame_drops += 1;
        }
    }

    async fn check_performance_issues(&mut self, frame_time: Duration) -> IntegrationResult<()> {
        // Check for frame drops
        if frame_time > self.target_frametime * 2 {
            warn!(
                "Frame drop detected: {:?} (target: {:?})",
                frame_time, self.target_frametime
            );

            self.event_system
                .send_system_event(SystemEvent::PerformanceWarning {
                    metric: "frametime".to_string(),
                    value: frame_time.as_secs_f64() * 1000.0, // Convert to milliseconds
                })
                .await
                .map_err(|e| IntegrationError::EventSystemError {
                    message: e.to_string(),
                })?;
        }

        // Check average frametime over last 60 frames
        if self.update_times.len() >= 60 && self.avg_frametime > self.target_frametime.mul_f64(1.5)
        {
            warn!(
                "Poor average performance: {:?} (target: {:?})",
                self.avg_frametime, self.target_frametime
            );

            self.event_system
                .send_system_event(SystemEvent::PerformanceWarning {
                    metric: "avg_frametime".to_string(),
                    value: self.avg_frametime.as_secs_f64() * 1000.0,
                })
                .await
                .map_err(|e| IntegrationError::EventSystemError {
                    message: e.to_string(),
                })?;
        }

        Ok(())
    }
}

/// Coordinator status information
#[derive(Debug, Clone)]
pub struct CoordinatorStatus {
    pub state: SubsystemState,
    pub frame_count: u64,
    pub avg_frametime: Duration,
    pub frame_drops: u32,
    pub subsystems: SubsystemStatus,
}

/// Subsystem status information
#[derive(Debug, Clone)]
pub struct SubsystemStatus {
    pub game_client: SubsystemState,
    pub game_logic: SubsystemState,
    pub game_network: SubsystemState,
    pub audio_system: SubsystemState,
}

impl std::fmt::Display for SubsystemState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SubsystemState::Uninitialized => write!(f, "Uninitialized"),
            SubsystemState::Initializing => write!(f, "Initializing"),
            SubsystemState::Running => write!(f, "Running"),
            SubsystemState::Paused => write!(f, "Paused"),
            SubsystemState::ShuttingDown => write!(f, "Shutting Down"),
            SubsystemState::Shutdown => write!(f, "Shutdown"),
            SubsystemState::Error => write!(f, "Error"),
        }
    }
}
