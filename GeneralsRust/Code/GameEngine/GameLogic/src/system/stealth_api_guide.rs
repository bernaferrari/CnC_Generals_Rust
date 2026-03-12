//! # Stealth & Detection System - Complete API Documentation
//!
//! This module provides comprehensive documentation, reference materials, and usage examples
//! for integrating Command & Conquer Generals Zero Hour's stealth and detection system into
//! game code.
//!
//! ## Architecture Overview
//!
//! The stealth system is architected as a collection of interconnected managers that handle
//! different aspects of stealth and detection gameplay:
//!
//! ```text
//! ┌────────────────────────────────────────────────────────────────┐
//! │                   Stealth & Detection System                    │
//! ├────────────────────────────────────────────────────────────────┤
//! │                                                                  │
//! │  ┌──────────────────────────────────────────────────────────┐  │
//! │  │         StealthManager (Stealth State)                   │  │
//! │  │  - Per-object stealth status (Hidden/Invisible/Revealed) │  │
//! │  │  - Per-player visibility tracking                        │  │
//! │  │  - Stealth strength values (0.0-100.0)                   │  │
//! │  │  - Reveal tracking                                       │  │
//! │  └──────────────────────────────────────────────────────────┘  │
//! │                              │                                  │
//! │  ┌──────────────────────────┴─────────────────────────────┐   │
//! │  │      Detection System (Finding Stealthed Units)         │   │
//! │  │                                                          │   │
//! │  │  ┌─────────────────────────────────────────────────┐   │   │
//! │  │  │ DetectionManager                                 │   │   │
//! │  │  │ - Detection strength per unit (0.0-100.0)       │   │   │
//! │  │  │ - Detection vs stealth comparison logic         │   │   │
//! │  └─────────────────────────────────────────────────────┘   │   │
//! │  │                                                          │   │
//! │  │  ┌─────────────────────────────────────────────────┐   │   │
//! │  │  │ DetectionModifierCalculator                      │   │   │
//! │  │  │ - Distance-based modifiers                       │   │   │
//! │  │  │ - Movement velocity modifiers                    │   │   │
//! │  │  │ - Unit type detection difficulty                │   │   │
//! │  │  │ - Garrisoned unit bonuses                        │   │   │
//! │  │  └─────────────────────────────────────────────────┘   │   │
//! │  │                                                          │   │
//! │  │  ┌─────────────────────────────────────────────────┐   │   │
//! │  │  │ DetectionEventManager                            │   │   │
//! │  │  │ - Radar events for UI                            │   │   │
//! │  │  │ - Audio feedback (quiet/loud ping)               │   │   │
//! │  │  │ - Particle system triggers                       │   │   │
//! │  │  │ - Eva messages                                   │   │   │
//! │  │  └─────────────────────────────────────────────────┘   │   │
//! │  └──────────────────────────────────────────────────────────┘  │
//! │                              │                                  │
//! │  ┌──────────────────────────┴──────────────────────────────┐  │
//! │  │      Stealth Conditions & Capabilities                  │  │
//! │  │                                                          │  │
//! │  │  ┌────────────────────────────────────────────────────┐ │  │
//! │  │  │ StealthConditionsManager                           │ │  │
//! │  │  │ - Tracks conditions breaking stealth              │ │  │
//! │  │  │ - Attacking, Moving, Firing, Taking Damage, etc.  │ │  │
//! │  │  └────────────────────────────────────────────────────┘ │  │
//! │  │                                                          │  │
//! │  │  ┌────────────────────────────────────────────────────┐ │  │
//! │  │  │ StealthUpgradeManager                              │ │  │
//! │  │  │ - Stealth capability grants                        │ │  │
//! │  │  │ - Black market integration                         │ │  │
//! │  │  │ - Tech tree registration                           │ │  │
//! │  │  └────────────────────────────────────────────────────┘ │  │
//! │  │                                                          │  │
//! │  │  ┌────────────────────────────────────────────────────┐ │  │
//! │  │  │ StealthSpecialPowerManager                         │ │  │
//! │  │  │ - Temporary stealth grants                         │ │  │
//! │  │  │ - Area-effect stealth zones                        │ │  │
//! │  │  │ - Spy vision effects                               │ │  │
//! │  │  └────────────────────────────────────────────────────┘ │  │
//! │  │                                                          │  │
//! │  │  ┌────────────────────────────────────────────────────┐ │  │
//! │  │  │ DisguiseManager                                    │ │  │
//! │  │  │ - Disguise template swapping                       │ │  │
//! │  │  │ - Per-player opacity tracking                      │ │  │
//! │  │  │ - Disguise transitions                             │ │  │
//! │  │  └────────────────────────────────────────────────────┘ │  │
//! │  └──────────────────────────────────────────────────────────┘  │
//! │                                                                  │
//! └────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Design Principles
//!
//! ### 1. Separation of Concerns
//! Each manager handles a single responsibility:
//! - **StealthManager**: Manages what stealth *is* (state, status)
//! - **DetectionManager**: Manages what can *detect* stealth
//! - **StealthConditions**: Manages what *breaks* stealth
//! - **Upgrades**: Manages stealth *capability*
//! - **Disguise**: Manages *appearance* while stealthed
//!
//! ### 2. Per-Player Visibility Independence
//! Stealth state is independent of fog-of-war. An object can be:
//! - Stealthed but visible to one player due to fog clearing
//! - In visible area but invisible to another player due to stealth
//! - Revealed to enemies but still stealthed to allies (team-based stealth)
//!
//! ### 3. Bit-Flag Efficiency
//! Stealth conditions use bitmasks (u16) to track 9 conditions simultaneously.
//! This allows fast checking: `if conditions & MOVING != 0` to test multiple flags.
//!
//! ### 4. Pluggable Modifiers
//! Detection can be affected by many factors without changing core detection logic:
//! - Distance decay
//! - Unit movement speed
//! - Unit type difficulty
//! - Garrisoned unit bonuses
//! - Line-of-sight effects
//!
//! ### 5. Event-Driven Architecture
//! Detection events are collected and dispatched for:
//! - UI feedback (radar events)
//! - Audio playback (pings)
//! - Particle effects (IR indicators)
//! - Eva messages (voice feedback)
//!
//! ## Thread Safety
//!
//! All managers use `Mutex<T>` for thread-safe interior mutability:
//! - **Zero-copy access**: Only lock when modifying state
//! - **Minimal lock duration**: Lock scope is as small as possible
//! - **No recursive locking**: Never hold multiple locks
//! - **Panic safety**: Poisoned locks are recoverable via error handling
//!
//! Example safe access pattern:
//! ```rust,ignore
//! let manager = STEALTH_MANAGER.lock()?;
//! let status = manager.get_stealth_status(object_id, player_id)?;
//! drop(manager); // Explicitly drop to release lock early
//! // Now safe to call other managers that might use locks
//! ```
//!
//! ## Performance Characteristics
//!
//! | Operation | Complexity | Notes |
//! |-----------|-----------|-------|
//! | Get stealth status | O(1) | HashMap lookup with player index |
//! | Set stealth status | O(1) | Direct state modification |
//! | Check condition flags | O(1) | Bitwise AND operation |
//! | Add detection modifier | O(1) | Append to modifier list |
//! | Calculate full modifier | O(n) | n = number of active modifiers |
//! | Batch detect objects | O(n*m) | n objects × m potential detectors |
//! | Event collection | O(1) amortized | VecDeque with pre-allocated capacity |
//!
//! Optimization strategies documented in the "Performance Guidelines" section.
//!
//! ---
//!
//! # API Reference
//!
//! ## StealthManager - Core Stealth State
//!
//! Manages the fundamental stealth state for all game objects.
//!
//! ### Key Types
//!
//! ```rust,ignore
//! use crate::system::stealth_manager::{
//!     StealthManager,
//!     StealthStatus,      // Hidden, Invisible, Revealed
//!     StealthStrength,    // 0.0-100.0 effectiveness
//! };
//! use crate::common::ObjectID;
//!
//! // StealthStatus enum variants:
//! // - Hidden: Normal visibility (not stealthed)
//! // - Invisible: Stealthed (requires detection to see)
//! // - Revealed: Stealth broken (visible regardless)
//!
//! // StealthStrength helpers:
//! let weak = StealthStrength::weak_stealth();           // 30.0
//! let standard = StealthStrength::standard_cloak();     // 60.0
//! let strong = StealthStrength::strong_stealth();       // 90.0
//! let custom = StealthStrength::new(75.5);             // Custom value
//! ```
//!
//! ### Common Operations
//!
//! #### Initialize Stealth for an Object
//! ```rust,ignore
//! use crate::system::stealth_manager::StealthManager;
//! use crate::common::ObjectID;
//!
//! // Get the global stealth manager instance
//! let manager = STEALTH_MANAGER.lock()?;
//!
//! // Initialize stealth for a new unit
//! let object_id: ObjectID = 42;
//! manager.register_object(object_id)?;
//!
//! // Set initial stealth state (Invisible with 60.0 strength)
//! manager.set_stealth_status(
//!     object_id,
//!     StealthStatus::Invisible,
//!     StealthStrength::standard_cloak(),
//! )?;
//! ```
//!
//! #### Check if Object is Stealthed
//! ```rust,ignore
//! // Check per-player visibility (crucial for multiplayer)
//! let player_id: usize = 0;  // Check for player 0
//! let status = manager.get_stealth_status(object_id, player_id)?;
//!
//! let is_stealthed = matches!(status, StealthStatus::Invisible);
//! if is_stealthed {
//!     // Don't render this unit for this player
//!     println!("Unit {} is stealthed to player {}", object_id, player_id);
//! }
//! ```
//!
//! #### Reveal a Stealthed Unit
//! ```rust,ignore
//! // Reveal to a specific player (they now see through stealth)
//! manager.reveal_to_player(object_id, player_id, "detected by radar")?;
//!
//! // Or reveal to everyone
//! for player in 0..8 {
//!     manager.reveal_to_player(object_id, player, "detected")?;
//! }
//! ```
//!
//! #### Change Stealth Strength
//! ```rust,ignore
//! // Upgrade to stronger stealth mid-game
//! manager.set_stealth_strength(
//!     object_id,
//!     StealthStrength::strong_stealth(),
//! )?;
//! ```
//!
//! ### API Summary
//!
//! | Method | Input | Output | Purpose |
//! |--------|-------|--------|---------|
//! | `register_object` | ObjectID | StealthResult<()> | Initialize stealth tracking |
//! | `unregister_object` | ObjectID | StealthResult<()> | Stop tracking (cleanup) |
//! | `set_stealth_status` | ObjectID, Status, Strength | StealthResult<()> | Set stealth state |
//! | `get_stealth_status` | ObjectID, Player | StealthResult<StealthStatus> | Query per-player visibility |
//! | `set_stealth_strength` | ObjectID, Strength | StealthResult<()> | Change stealth effectiveness |
//! | `get_stealth_strength` | ObjectID | StealthResult<StealthStrength> | Query current strength |
//! | `reveal_to_player` | ObjectID, Player, Reason | StealthResult<()> | Break stealth for player |
//! | `reveal_to_all` | ObjectID, Reason | StealthResult<()> | Break stealth universally |
//! | `is_revealed` | ObjectID, Player | StealthResult<bool> | Check if visible |
//!
//! ---
//!
//! ## DetectionManager - Detection Capabilities
//!
//! Manages detection strength and performs stealth detection calculations.
//!
//! ### Key Types
//!
//! ```rust,ignore
//! use crate::system::detection_manager::{
//!     DetectionManager,
//!     DetectionStrength,     // 0.0-100.0 detection capability
//!     DetectionModifier,     // Multiplicative factors
//! };
//!
//! // DetectionStrength helpers:
//! let weak = DetectionStrength::weak_detector();        // 30.0
//! let standard = DetectionStrength::standard_detector(); // 60.0
//! let strong = DetectionStrength::strong_detector();    // 90.0
//!
//! // DetectionModifier structure:
//! // - distance_factor: How distance affects detection (0.0-1.0)
//! // - unit_type_factor: Type difficulty (0.0-1.0)
//! // - movement_factor: Movement penalty (0.0-1.0)
//! // - special_factor: Special abilities (0.0-1.0)
//! ```
//!
//! ### Common Operations
//!
//! #### Set Unit Detection Strength
//! ```rust,ignore
//! use crate::system::detection_manager::DetectionManager;
//!
//! let manager = DETECTION_MANAGER.lock()?;
//!
//! // Infantry units are decent detectors
//! manager.set_detection_strength(
//!     detector_unit_id,
//!     DetectionStrength::standard_detector(),
//! )?;
//!
//! // Anti-cloak units are excellent detectors
//! manager.set_detection_strength(
//!     anti_stealth_unit_id,
//!     DetectionStrength::strong_detector(),
//! )?;
//! ```
//!
//! #### Test if Stealth Unit is Detected
//! ```rust,ignore
//! // Core detection algorithm: detection_strength * modifiers > stealth_strength
//! let can_detect = manager.can_detect(
//!     detector_id,
//!     stealthed_unit_id,
//!     modifier, // See DetectionModifierCalculator
//! )?;
//!
//! if can_detect {
//!     println!("Unit {} was detected by {}!", stealthed_unit_id, detector_id);
//!     // Fire detection events, update radar, play sounds, etc.
//! }
//! ```
//!
//! #### Query Detection Strength
//! ```rust,ignore
//! let strength = manager.get_detection_strength(detector_id)?;
//! println!("Detection strength: {}", strength.value());
//! ```
//!
//! ### API Summary
//!
//! | Method | Input | Output | Purpose |
//! |--------|-------|--------|---------|
//! | `register_detector` | ObjectID | StealthResult<()> | Track as detection unit |
//! | `unregister_detector` | ObjectID | StealthResult<()> | Stop tracking |
//! | `set_detection_strength` | ObjectID, Strength | StealthResult<()> | Set detection capability |
//! | `get_detection_strength` | ObjectID | StealthResult<DetectionStrength> | Query capability |
//! | `can_detect` | Detector, Target, Modifier | StealthResult<bool> | Test if detected |
//! | `get_detection_difficulty` | Target | StealthResult<f32> | Query target difficulty |
//!
//! ---
//!
//! ## StealthConditionsManager - Stealth Availability
//!
//! Manages conditions that break or prevent stealth.
//!
//! ### Key Types
//!
//! ```rust,ignore
//! use crate::system::stealth_conditions::{
//!     StealthConditionsManager,
//!     StealthCondition,          // Enum: Attacking, Moving, Firing, etc.
//!     StealthConditionFlags,     // u16 bitmask
//! };
//!
//! // Condition variants (9 total):
//! pub enum StealthCondition {
//!     Attacking,           // Unit is attacking (bit 0)
//!     Moving,              // Unit is moving (bit 1)
//!     UsingAbility,        // Unit is using ability (bit 2)
//!     FiringPrimary,       // Primary weapon firing (bit 3)
//!     FiringSecondary,     // Secondary weapon firing (bit 4)
//!     FiringTertiary,      // Tertiary weapon firing (bit 5)
//!     NoBlackMarket,       // Black market unavailable (bit 6)
//!     TakingDamage,        // Currently damaged (bit 7)
//!     RidersAttacking,     // Rider units attacking (bit 8)
//! }
//! ```
//!
//! ### Common Operations
//!
//! #### Set Conditions Blocking Stealth
//! ```rust,ignore
//! use crate::system::stealth_conditions::StealthConditionsManager;
//!
//! let manager = CONDITIONS_MANAGER.lock()?;
//!
//! // Unit is attacking, so stealth is broken
//! manager.set_condition(unit_id, StealthCondition::Attacking, true)?;
//!
//! // Unit started moving, add moving condition
//! manager.set_condition(unit_id, StealthCondition::Moving, true)?;
//! ```
//!
//! #### Check if Unit Can Be Stealthed
//! ```rust,ignore
//! // Check if any condition prevents stealth
//! let can_be_stealthed = manager.can_maintain_stealth(unit_id)?;
//!
//! if !can_be_stealthed {
//!     // Units break stealth under certain conditions
//!     let broken_by = manager.get_breaking_condition(unit_id)?;
//!     println!("Stealth broken by: {:?}", broken_by);
//! }
//! ```
//!
//! #### Clear Conditions When Action Stops
//! ```rust,ignore
//! // Unit stopped attacking
//! manager.set_condition(unit_id, StealthCondition::Attacking, false)?;
//!
//! // Unit stopped moving
//! manager.set_condition(unit_id, StealthCondition::Moving, false)?;
//!
//! // Can now re-stealth if no other conditions active
//! ```
//!
//! #### Batch Check Conditions
//! ```rust,ignore
//! // Query all active conditions
//! let flags = manager.get_condition_flags(unit_id)?;
//!
//! // Check multiple conditions at once
//! let attacking = (flags & StealthCondition::Attacking.bitmask()) != 0;
//! let moving = (flags & StealthCondition::Moving.bitmask()) != 0;
//! let firing = (flags & StealthCondition::FiringPrimary.bitmask()) != 0;
//!
//! if attacking || moving || firing {
//!     println!("Stealth is prevented!");
//! }
//! ```
//!
//! ### API Summary
//!
//! | Method | Input | Output | Purpose |
//! |--------|-------|--------|---------|
//! | `register_object` | ObjectID | StealthResult<()> | Start tracking conditions |
//! | `unregister_object` | ObjectID | StealthResult<()> | Stop tracking |
//! | `set_condition` | ObjectID, Condition, bool | StealthResult<()> | Set/clear condition flag |
//! | `get_condition_flags` | ObjectID | StealthResult<u16> | Query all conditions |
//! | `can_maintain_stealth` | ObjectID | StealthResult<bool> | Check if stealthable |
//! | `get_breaking_condition` | ObjectID | StealthResult<Option<StealthCondition>> | Find preventing condition |
//! | `clear_all_conditions` | ObjectID | StealthResult<()> | Reset all flags |
//!
//! ---
//!
//! ## DisguiseManager - Appearance Cloaking
//!
//! Manages visual disguise rendering distinct from stealth detection.
//!
//! ### Key Types
//!
//! ```rust,ignore
//! use crate::system::disguise_manager::{
//!     DisguiseManager,
//!     DisguiseState,      // Per-object disguise data
//! };
//!
//! // DisguiseState contains:
//! // - disguised_as_template: What unit template to render as
//! // - disguised_as_player: Appear as team (Some(2)) or not (None)
//! // - per_player_opacity: 0.0 (invisible to allies) to 1.0 (visible to enemies)
//! // - transition_frames_remaining: Animation frame count
//! // - is_transitioning: Animation in progress flag
//! ```
//!
//! ### Common Operations
//!
//! #### Apply Disguise Rendering
//! ```rust,ignore
//! use crate::system::disguise_manager::DisguiseManager;
//!
//! let manager = DISGUISE_MANAGER.lock()?;
//!
//! // Unit disguises as American ranger
//! manager.apply_disguise(
//!     unit_id,
//!     "AmericanRanger",      // Template to render as
//!     Some(0),               // Appear as player 0 (team)
//!     30,                    // 30-frame transition
//! )?;
//! ```
//!
//! #### Check Per-Player Visibility
//! ```rust,ignore
//! // Check if unit appears disguised to a specific player
//! let opacity = manager.get_per_player_opacity(unit_id, player_id)?;
//!
//! if opacity < 1.0 {
//!     // Unit is partially or fully invisible to this player
//!     println!("Rendering at opacity: {}", opacity);
//! }
//! ```
//!
//! #### Update Disguise Transition
//! ```rust,ignore
//! // Called each frame during animation
//! manager.update_disguise_transition(unit_id)?;
//!
//! // Transition complete when frames reach 0
//! ```
//!
//! #### Remove Disguise
//! ```rust,ignore
//! // Revert to real appearance
//! manager.remove_disguise(unit_id)?;
//! ```
//!
//! ### API Summary
//!
//! | Method | Input | Output | Purpose |
//! |--------|-------|--------|---------|
//! | `apply_disguise` | ObjectID, Template, Player, Frames | StealthResult<()> | Start disguise |
//! | `remove_disguise` | ObjectID | StealthResult<()> | End disguise |
//! | `get_disguise_template` | ObjectID | StealthResult<String> | Query render template |
//! | `get_per_player_opacity` | ObjectID, Player | StealthResult<f32> | Query visibility |
//! | `set_per_player_opacity` | ObjectID, Player, Opacity | StealthResult<()> | Set opacity |
//! | `is_transitioning` | ObjectID | StealthResult<bool> | Check animation state |
//! | `update_disguise_transition` | ObjectID | StealthResult<()> | Advance animation |
//! | `set_team_wide_disguise` | ObjectID, bool | StealthResult<()> | Enable team-wide mode |
//!
//! ---
//!
//! ## StealthSpecialPowerManager - Temporary Stealth Grants
//!
//! Manages temporary stealth from special powers and abilities.
//!
//! ### Key Types
//!
//! ```rust,ignore
//! use crate::system::stealth_special_power::{
//!     StealthSpecialPowerManager,
//!     StealthGrant,               // Stealth grant record
//!     PERMANENT_STEALTH,          // Constant: -1
//! };
//!
//! // StealthGrant contains:
//! // - granted_to_id: Unit receiving stealth
//! // - granted_by_id: Unit/power granting stealth
//! // - frames_remaining: Duration (-1 = permanent)
//! // - is_area_effect: Zone-based stealth (e.g., cloak field)
//! ```
//!
//! ### Common Operations
//!
//! #### Grant Temporary Stealth from Special Power
//! ```rust,ignore
//! use crate::system::stealth_special_power::StealthSpecialPowerManager;
//!
//! let manager = SPECIAL_POWER_MANAGER.lock()?;
//!
//! // Grant 30 seconds (900 frames @ 30fps) of stealth
//! manager.grant_temporary_stealth(
//!     target_unit_id,
//!     special_power_object_id,
//!     900,  // Duration in frames
//! )?;
//! ```
//!
//! #### Grant Permanent Stealth Upgrade
//! ```rust,ignore
//! // Permanent stealth (e.g., from tech tree)
//! manager.grant_permanent_stealth(
//!     unit_id,
//!     upgrade_source_id,
//! )?;
//! ```
//!
//! #### Create Area-Effect Stealth Zone
//! ```rust,ignore
//! // All units in area of cloak generator become stealthed
//! manager.grant_area_effect_stealth(
//!     cloak_generator_id,
//!     radius,            // Zone radius in game units
//!     duration_frames,   // How long effect lasts
//!     strength,          // Stealth strength for affected units
//! )?;
//! ```
//!
//! #### Update Stealth Grants Each Frame
//! ```rust,ignore
//! // Called during game loop update
//! manager.update_stealth_grants()?;
//!
//! // Expired temporary grants are automatically removed
//! ```
//!
//! #### Check Active Stealth Grants
//! ```rust,ignore
//! let grants = manager.get_active_grants(unit_id)?;
//! for grant in grants {
//!     if grant.is_permanent() {
//!         println!("Permanent stealth granted by: {}", grant.granted_by_id);
//!     } else {
//!         println!("Temporary stealth, {} frames remaining", grant.frames_remaining);
//!     }
//! }
//! ```
//!
//! ### API Summary
//!
//! | Method | Input | Output | Purpose |
//! |--------|-------|--------|---------|
//! | `grant_temporary_stealth` | Target, Source, Duration | StealthResult<()> | Temporary grant |
//! | `grant_permanent_stealth` | Target, Source | StealthResult<()> | Permanent grant |
//! | `grant_area_effect_stealth` | Source, Radius, Duration, Strength | StealthResult<()> | Zone stealth |
//! | `revoke_stealth_grant` | Target, Source | StealthResult<()> | End grant |
//! | `update_stealth_grants` | None | StealthResult<()> | Expire temporary grants |
//! | `get_active_grants` | ObjectID | StealthResult<Vec<StealthGrant>> | Query grants |
//! | `grant_spy_vision` | Target, Duration, KindOf | StealthResult<()> | Special vision |
//!
//! ---
//!
//! ## StealthUpgradeManager - Tech Tree Integration
//!
//! Manages stealth capability upgrades from tech tree and black market.
//!
//! ### Key Types
//!
//! ```rust,ignore
//! use crate::system::stealth_upgrade::{
//!     StealthUpgradeManager,
//!     StealthUpgrade,        // Upgrade configuration
//! };
//!
//! // StealthUpgrade contains:
//! // - upgrade_name: Identifier (e.g., "AdvancedCloak")
//! // - applies_to_kindof: Unit type mask for applicability
//! // - grants_capability_to_spawned: Whether new units get capability
//! // - black_market_only: Requires black market availability
//! // - granted_by_tech: Tech tree reference
//! ```
//!
//! ### Common Operations
//!
//! #### Register Stealth Upgrade
//! ```rust,ignore
//! use crate::system::stealth_upgrade::StealthUpgradeManager;
//!
//! let manager = UPGRADE_MANAGER.lock()?;
//!
//! // Register upgrade configuration
//! manager.register_upgrade(
//!     "GhostStealth",        // Upgrade name
//!     0x00000004,            // Applies to infantry (bit 2)
//!     true,                  // New units get capability
//!     false,                 // Available normally (not black market)
//!     "GhostStealthTech",    // Tech tree reference
//! )?;
//! ```
//!
//! #### Grant Upgrade to Unit
//! ```rust,ignore
//! // Grant registered upgrade to specific unit
//! manager.grant_upgrade_to_unit(unit_id, "GhostStealth")?;
//!
//! // Unit can now stealth (if all other conditions met)
//! ```
//!
//! #### Grant Upgrade to All Compatible Units
//! ```rust,ignore
//! // Player researched technology, grant to all matching units
//! manager.grant_upgrade_to_player(player_id, "GhostStealth")?;
//! ```
//!
//! #### Check if Unit Has Capability
//! ```rust,ignore
//! let has_capability = manager.unit_has_stealth_capability(unit_id)?;
//! if has_capability {
//!     // Unit can enter stealth (conditions permitting)
//! }
//! ```
//!
//! #### Manage Black Market Availability
//! ```rust,ignore
//! // Enable black market for player 2
//! manager.set_black_market_available(2, true)?;
//!
//! // Check if player has black market (for upgrades)
//! let has_black_market = manager.is_black_market_available(2)?;
//! ```
//!
//! ### API Summary
//!
//! | Method | Input | Output | Purpose |
//! |--------|-------|--------|---------|
//! | `register_upgrade` | Name, KindOf, Spawned, Market, Tech | StealthResult<()> | Configure upgrade |
//! | `grant_upgrade_to_unit` | ObjectID, Upgrade | StealthResult<()> | Upgrade unit |
//! | `grant_upgrade_to_player` | Player, Upgrade | StealthResult<()> | Upgrade all compatible |
//! | `revoke_upgrade` | ObjectID, Upgrade | StealthResult<()> | Remove capability |
//! | `unit_has_stealth_capability` | ObjectID | StealthResult<bool> | Query capability |
//! | `set_black_market_available` | Player, bool | StealthResult<()> | Enable/disable market |
//! | `is_black_market_available` | Player | StealthResult<bool> | Query market status |
//! | `get_upgrade_info` | Upgrade | StealthResult<StealthUpgrade> | Query configuration |
//!
//! ---
//!
//! ## DetectionEventManager - Detection Feedback
//!
//! Manages events triggered by stealth detection for audio, visual, and UI feedback.
//!
//! ### Key Types
//!
//! ```rust,ignore
//! use crate::system::detection_events::{
//!     DetectionEventManager,
//!     DetectionEvent,            // Event enum
//!     DetectionEventType,        // Radar event type
//!     AudioEventType,            // Sound type
//!     EvaMessageType,            // Voice message
//! };
//!
//! // DetectionEventType variants:
//! // - RadarEventStealthDiscovered
//! // - QuietPing / LoudPing (sound feedback)
//! // - IRBeaconActivated, IRGridOverlay, IRPing, IRBright (particles)
//!
//! // DetectionEvent variants:
//! // - StealthDiscovered { object_id, detector_id, frame, player_id }
//! // - RadarEvent { object_id, event_type }
//! // - AudioEvent { sound_type, player_id, position }
//! // - EvaMessage { message_type, player_id }
//! ```
//!
//! ### Common Operations
//!
//! #### Fire Detection Event
//! ```rust,ignore
//! use crate::system::detection_events::DetectionEventManager;
//!
//! let manager = EVENT_MANAGER.lock()?;
//!
//! // Record stealth discovery
//! manager.record_detection_event(
//!     stealthed_unit_id,
//!     detector_unit_id,
//!     player_id,
//!     current_frame,
//! )?;
//! ```
//!
//! #### Fire Radar Event for UI
//! ```rust,ignore
//! manager.record_radar_event(
//!     unit_id,
//!     DetectionEventType::RadarEventStealthDiscovered,
//! )?;
//! ```
//!
//! #### Queue Audio Feedback
//! ```rust,ignore
//! manager.record_audio_event(
//!     AudioEventType::LoudPing,  // Confirmed detection
//!     player_id,
//!     unit_position,
//! )?;
//! ```
//!
//! #### Queue Eva Message
//! ```rust,ignore
//! manager.record_eva_message(
//!     EvaMessageType::EnemyDetected,
//!     player_id,
//! )?;
//! ```
//!
//! #### Process Events for Rendering
//! ```rust,ignore
//! // Called once per frame by rendering/audio systems
//! let events = manager.collect_events()?;
//!
//! for event in events {
//!     match event {
//!         DetectionEvent::StealthDiscovered { object_id, detector_id, .. } => {
//!             println!("Unit {} was detected by {}", object_id, detector_id);
//!         }
//!         DetectionEvent::RadarEvent { object_id, event_type } => {
//!             // Update radar display
//!         }
//!         DetectionEvent::AudioEvent { sound_type, .. } => {
//!             // Play audio feedback
//!         }
//!         DetectionEvent::EvaMessage { message_type, .. } => {
//!             // Queue voice message
//!         }
//!     }
//! }
//! ```
//!
//! ### API Summary
//!
//! | Method | Input | Output | Purpose |
//! |--------|-------|--------|---------|
//! | `record_detection_event` | Target, Detector, Player, Frame | StealthResult<()> | Detection event |
//! | `record_radar_event` | ObjectID, EventType | StealthResult<()> | Radar feedback |
//! | `record_audio_event` | AudioType, Player, Position | StealthResult<()> | Sound feedback |
//! | `record_eva_message` | MessageType, Player | StealthResult<()> | Voice message |
//! | `collect_events` | None | StealthResult<Vec<DetectionEvent>> | Drain all events |
//! | `get_event_count` | None | StealthResult<usize> | Query event count |
//! | `clear_events` | None | StealthResult<()> | Empty queue |
//!
//! ---
//!
//! ## DetectionModifierCalculator - Advanced Detection Calculations
//!
//! Calculates dynamic detection modifiers based on game state.
//!
//! ### Key Types
//!
//! ```rust,ignore
//! use crate::system::detection_modifiers::{
//!     DetectionModifierCalculator,
//!     DetectionModifier,         // Combined modifier result
//!     DistanceFalloffCurve,      // Linear, Exponential, Sigmoid
//!     UnitTypeCategory,          // Infantry, Vehicle, Aircraft, Building, Stealth
//! };
//!
//! // Falloff curves affect distance modifier calculation:
//! // - Linear: modifier = 1.0 - (distance / max_range)
//! // - Exponential: modifier = (1.0 - (distance / max_range))^2
//! // - Sigmoid: smooth S-curve transition
//!
//! // UnitTypeCategory difficulty multipliers:
//! // - Infantry: 1.0x (baseline)
//! // - Vehicle: 0.8x (easier to detect)
//! // - Aircraft: 1.2x (harder to detect)
//! // - Building: 1.5x (very hard)
//! // - Stealth: 0.5x (extremely hard)
//! ```
//!
//! ### Common Operations
//!
//! #### Calculate Combined Detection Modifier
//! ```rust,ignore
//! use crate::system::detection_modifiers::DetectionModifierCalculator;
//!
//! let calculator = MODIFIER_CALCULATOR.lock()?;
//!
//! // Calculate how easy this unit is to detect given all factors
//! let modifier = calculator.calculate_detection_modifier(
//!     stealthed_unit_id,
//!     detector_unit_id,
//!     distance_between_units,
//!     target_velocity,
//! )?;
//!
//! // Use in detection calculation:
//! // effective_detection = detector_strength * modifier > stealth_strength
//! ```
//!
//! #### Calculate Distance Factor Only
//! ```rust,ignore
//! let distance_mod = calculator.calculate_distance_modifier(
//!     distance,
//!     max_detection_range,
//!     falloff_curve,
//! )?;
//!
//! // Value between 0.0 (far) and 1.0 (close)
//! ```
//!
//! #### Calculate Movement Factor
//! ```rust,ignore
//! // Faster units are easier to detect
//! let movement_mod = calculator.calculate_movement_modifier(
//!     current_velocity,
//!     threshold_velocity,
//! )?;
//! ```
//!
//! #### Get Unit Type Detection Difficulty
//! ```rust,ignore
//! use crate::system::detection_modifiers::UnitTypeCategory;
//!
//! let difficulty = UnitTypeCategory::Aircraft.difficulty_multiplier();
//! // 1.2x harder to detect than baseline infantry
//! ```
//!
//! ### API Summary
//!
//! | Method | Input | Output | Purpose |
//! |--------|-------|--------|---------|
//! | `calculate_detection_modifier` | Target, Detector, Distance, Velocity | Result<DetectionModifier> | Full calculation |
//! | `calculate_distance_modifier` | Distance, MaxRange, Curve | Result<f32> | Distance only |
//! | `calculate_movement_modifier` | Velocity, Threshold | Result<f32> | Movement only |
//! | `get_unit_type_difficulty` | UnitType | Result<f32> | Type difficulty |
//! | `get_rider_modifier` | ContainerID | Result<f32> | Rider penalty |
//! | `get_garrisoned_modifier` | BuildingID | Result<f32> | Garrison bonus |
//! | `apply_weather_modifier` | WeatherType | Result<f32> | Environmental factor |
//!
//! ---
//!
//! # Usage Examples
//!
//! ## Example 1: Initializing Stealth for an Object
//!
//! ```rust,ignore
//! use crate::system::stealth_manager::{StealthManager, StealthStatus, StealthStrength};
//! use crate::common::ObjectID;
//!
//! fn initialize_stealthed_unit(unit_id: ObjectID) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//!     let stealth_mgr = STEALTH_MANAGER.lock()?;
//!
//!     // Register the unit with stealth system
//!     stealth_mgr.register_object(unit_id)?;
//!
//!     // Set initial state: invisible with standard stealth strength
//!     stealth_mgr.set_stealth_status(
//!         unit_id,
//!         StealthStatus::Invisible,
//!         StealthStrength::standard_cloak(),
//!     )?;
//!
//!     println!("Unit {} initialized for stealth", unit_id);
//!     Ok(())
//! }
//! ```
//!
//! ## Example 2: Checking Stealth Status Per Player
//!
//! ```rust,ignore
//! use crate::system::stealth_manager::StealthStatus;
//!
//! fn should_render_unit(unit_id: ObjectID, player_id: usize) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
//!     let stealth_mgr = STEALTH_MANAGER.lock()?;
//!
//!     let status = stealth_mgr.get_stealth_status(unit_id, player_id)?;
//!
//!     Ok(matches!(status, StealthStatus::Hidden | StealthStatus::Revealed))
//! }
//! ```
//!
//! ## Example 3: Detecting Stealthed Units
//!
//! ```rust,ignore
//! use crate::system::detection_manager::DetectionManager;
//! use crate::system::detection_modifiers::DetectionModifierCalculator;
//!
//! fn check_detection(
//!     detector_id: ObjectID,
//!     target_id: ObjectID,
//!     distance: f32,
//! ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
//!     let detection_mgr = DETECTION_MANAGER.lock()?;
//!     let calc = MODIFIER_CALCULATOR.lock()?;
//!
//!     // Calculate dynamic modifiers
//!     let modifier = calc.calculate_detection_modifier(
//!         target_id,
//!         detector_id,
//!         distance,
//!         0.0,  // Assume stationary for this example
//!     )?;
//!
//!     // Test detection with calculated modifiers
//!     detection_mgr.can_detect(detector_id, target_id, &modifier)
//! }
//! ```
//!
//! ## Example 4: Processing Detection Events
//!
//! ```rust,ignore
//! use crate::system::detection_events::{DetectionEvent, DetectionEventManager};
//!
//! fn process_detection_this_frame() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//!     let event_mgr = EVENT_MANAGER.lock()?;
//!
//!     let events = event_mgr.collect_events()?;
//!
//!     for event in events {
//!         match event {
//!             DetectionEvent::StealthDiscovered { object_id, detector_id, player_id, .. } => {
//!                 println!("Player {} detected unit {} via {}", player_id, object_id, detector_id);
//!                 // Trigger radar update, sounds, eva message
//!             }
//!             DetectionEvent::AudioEvent { sound_type, player_id, .. } => {
//!                 // Play detection sound for player
//!                 play_detection_sound(player_id, sound_type)?;
//!             }
//!             DetectionEvent::EvaMessage { message_type, player_id } => {
//!                 // Queue eva voice message
//!                 queue_eva_message(player_id, message_type)?;
//!             }
//!             _ => {}
//!         }
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Example 5: Handling Special Power Grants
//!
//! ```rust,ignore
//! use crate::system::stealth_special_power::StealthSpecialPowerManager;
//!
//! fn grant_temporary_stealth_special_power(
//!     affected_units: &[ObjectID],
//!     duration_frames: i32,
//!     power_source: ObjectID,
//! ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//!     let power_mgr = SPECIAL_POWER_MANAGER.lock()?;
//!
//!     for unit_id in affected_units {
//!         power_mgr.grant_temporary_stealth(*unit_id, power_source, duration_frames)?;
//!     }
//!
//!     println!("Granted stealth to {} units for {} frames",
//!              affected_units.len(), duration_frames);
//!     Ok(())
//! }
//! ```
//!
//! ## Example 6: Applying Upgrades from Tech Tree
//!
//! ```rust,ignore
//! use crate::system::stealth_upgrade::StealthUpgradeManager;
//!
//! fn research_stealth_upgrade(
//!     player_id: u32,
//!     upgrade_name: &str,
//! ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//!     let upgrade_mgr = UPGRADE_MANAGER.lock()?;
//!
//!     // Grant to all units of compatible type for this player
//!     upgrade_mgr.grant_upgrade_to_player(player_id, upgrade_name)?;
//!
//!     // Future spawned units will also get capability
//!
//!     println!("Player {} researched {}", player_id, upgrade_name);
//!     Ok(())
//! }
//! ```
//!
//! ---
//!
//! # Integration Patterns
//!
//! ## Game Loop Integration
//!
//! ```rust,ignore
//! fn game_loop_update() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//!     // Phase 1: Update game state
//!     update_unit_positions()?;
//!     update_unit_weapons()?;
//!
//!     // Phase 2: Update stealth conditions based on new state
//!     for unit in all_units() {
//!         if unit.is_attacking {
//!             CONDITIONS_MANAGER.lock()?
//!                 .set_condition(unit.id, StealthCondition::Attacking, true)?;
//!         }
//!         if unit.velocity > 0.1 {
//!             CONDITIONS_MANAGER.lock()?
//!                 .set_condition(unit.id, StealthCondition::Moving, true)?;
//!         }
//!     }
//!
//!     // Phase 3: Process stealth grants (expiration, area effects)
//!     SPECIAL_POWER_MANAGER.lock()?
//!         .update_stealth_grants()?;
//!
//!     // Phase 4: Perform detection checks
//!     perform_detection_sweep()?;
//!
//!     // Phase 5: Render (use stealth visibility per player)
//!     render_game()?;
//!
//!     // Phase 6: Dispatch detection events
//!     process_detection_events()?;
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Rendering System Integration
//!
//! ```rust,ignore
//! fn render_unit(unit: &Unit, player_id: usize) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//!     let stealth_mgr = STEALTH_MANAGER.lock()?;
//!     let disguise_mgr = DISGUISE_MANAGER.lock()?;
//!
//!     // Check if unit should be visible to this player
//!     let status = stealth_mgr.get_stealth_status(unit.id, player_id)?;
//!
//!     match status {
//!         StealthStatus::Revealed => {
//!             // Unit is revealed, render normally
//!             render_unit_model(&unit.template, unit.position)?;
//!         }
//!         StealthStatus::Invisible => {
//!             // Unit is stealthed, don't render (or render as shimmer)
//!             render_stealth_shimmer(unit.position)?;
//!         }
//!         StealthStatus::Hidden => {
//!             // Normal unit, check for disguise
//!             let template = disguise_mgr.get_disguise_template(unit.id)
//!                 .unwrap_or_else(|_| unit.template.clone());
//!             let opacity = disguise_mgr.get_per_player_opacity(unit.id, player_id)?;
//!             render_unit_model(&template, unit.position)?;
//!             apply_opacity(opacity)?;
//!         }
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! ## AI Detection System Integration
//!
//! ```rust,ignore
//! fn ai_should_attack(ai_unit: &Unit, target: &Unit) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
//!     let detection_mgr = DETECTION_MANAGER.lock()?;
//!     let modifier_calc = MODIFIER_CALCULATOR.lock()?;
//!
//!     // Can only attack targets we can detect
//!     if !target.is_stealthed {
//!         return Ok(true);  // Can see normal units
//!     }
//!
//!     // Target is stealthed, check if we can detect
//!     let distance = ai_unit.position.distance_to(target.position);
//!     let modifier = modifier_calc.calculate_detection_modifier(
//!         target.id, ai_unit.id, distance, target.velocity
//!     )?;
//!
//!     detection_mgr.can_detect(ai_unit.id, target.id, &modifier)
//! }
//! ```
//!
//! ## Event Handling Pattern
//!
//! ```rust,ignore
//! fn on_unit_detected(detector_id: ObjectID, target_id: ObjectID) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//!     let event_mgr = EVENT_MANAGER.lock()?;
//!     let stealth_mgr = STEALTH_MANAGER.lock()?;
//!
//!     // Get detector's player
//!     let detector = get_unit(detector_id)?;
//!     let target = get_unit(target_id)?;
//!
//!     // Record detection event
//!     event_mgr.record_detection_event(
//!         target_id,
//!         detector_id,
//!         detector.player as usize,
//!         current_frame,
//!     )?;
//!
//!     // Reveal to detector's team
//!     for player in detector.get_team_members() {
//!         stealth_mgr.reveal_to_player(target_id, player as usize, "detected by team")?;
//!     }
//!
//!     // Fire audio and radar events
//!     event_mgr.record_audio_event(
//!         AudioEventType::LoudPing,
//!         detector.player as usize,
//!         detector.position,
//!     )?;
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Error Handling Best Practices
//!
//! ```rust,ignore
//! use crate::system::stealth_errors::StealthError;
//!
//! fn safe_stealth_operation() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//!     // Pattern 1: Check for specific error types
//!     match STEALTH_MANAGER.lock() {
//!         Ok(mgr) => {
//!             match mgr.set_stealth_status(unit_id, status, strength) {
//!                 Ok(_) => println!("Stealth set successfully"),
//!                 Err(StealthError::ObjectNotRegistered { object_id }) => {
//!                     eprintln!("Unit {} not registered for stealth", object_id);
//!                     // Handle unregistered object case
//!                 }
//!                 Err(e) => {
//!                     eprintln!("Stealth operation failed: {}", e);
//!                     // Log and potentially recover
//!                 }
//!             }
//!         }
//!         Err(StealthError::LockPoisoned { module }) => {
//!             eprintln!("Lock poisoned in: {}", module);
//!             // Attempt recovery or abort gracefully
//!         }
//!         Err(e) => eprintln!("Failed to acquire lock: {}", e),
//!     }
//!
//!     // Pattern 2: Use ? operator for early returns
//!     let mgr = STEALTH_MANAGER.lock()?;
//!     mgr.register_object(unit_id)?;
//!     mgr.set_stealth_status(unit_id, status, strength)?;
//!     // If any operation fails, error propagates up
//!
//!     Ok(())
//! }
//! ```
//!
//! ---
//!
//! # Performance Guidelines
//!
//! ## Caching Strategies
//!
//! **Problem**: Repeatedly acquiring locks for frequently-accessed data is slow.
//!
//! **Solution**: Cache results locally during hot code paths:
//! ```rust,ignore
//! // GOOD: Cache stealth status before rendering loop
//! let stealth_mgr = STEALTH_MANAGER.lock()?;
//! let mut status_cache = HashMap::new();
//! for unit in visible_units {
//!     status_cache.insert(unit.id,
//!         stealth_mgr.get_stealth_status(unit.id, current_player)?);
//! }
//! drop(stealth_mgr);  // Release lock
//!
//! for unit in visible_units {
//!     if let Some(status) = status_cache.get(&unit.id) {
//!         // Use cached status, no lock needed
//!     }
//! }
//!
//! // BAD: Acquiring lock repeatedly in loop
//! for unit in visible_units {
//!     let status = STEALTH_MANAGER.lock()?.get_stealth_status(...)?; // Lock each iteration!
//! }
//! ```
//!
//! ## Lock Contention Avoidance
//!
//! **Problem**: Multiple threads waiting for same lock hurts performance.
//!
//! **Solution**: Minimize lock-holding time:
//! ```rust,ignore
//! // GOOD: Quick lock-and-release pattern
//! let result = {
//!     let mgr = STEALTH_MANAGER.lock()?;
//!     mgr.get_stealth_status(unit_id, player_id)?
//! };  // Lock released here
//! do_expensive_computation(result)?;  // No lock held
//!
//! // BAD: Holding lock during expensive operation
//! let mgr = STEALTH_MANAGER.lock()?;
//! let status = mgr.get_stealth_status(unit_id, player_id)?;
//! do_expensive_computation(status)?;  // Lock still held!
//! drop(mgr);
//! ```
//!
//! ## Memory Usage Optimization
//!
//! **Problem**: Storing unnecessary data per-object wastes memory.
//!
//! **Solution**: Use efficient representations:
//! ```rust,ignore
//! // GOOD: Use bit flags for conditions
//! let conditions: u16 = bitmask;  // 9 conditions in 2 bytes
//!
//! // BAD: Store conditions as individual booleans
//! struct Conditions {
//!     attacking: bool,       // 1 byte
//!     moving: bool,          // 1 byte
//!     using_ability: bool,   // 1 byte
//!     ...                    // 9 bytes per object!
//! }
//! ```
//!
//! ## Modifier Calculation Optimization
//!
//! **Problem**: Recalculating modifiers every frame for every detector-target pair is slow.
//!
//! **Solution**: Cache or batch calculations:
//! ```rust,ignore
//! // GOOD: Batch detection checks
//! let detector_positions = collect_detector_positions();
//! let target_positions = collect_target_positions();
//! let modifier_calc = MODIFIER_CALCULATOR.lock()?;
//!
//! for (detector_id, detector_pos) in detector_positions {
//!     for (target_id, target_pos) in target_positions {
//!         let distance = detector_pos.distance_to(target_pos);
//!         let modifier = modifier_calc.calculate_detection_modifier(...)?;
//!         // Use modifier for detection check
//!     }
//! }
//!
//! // BAD: Recalculating from scratch repeatedly
//! let modifier_calc = MODIFIER_CALCULATOR.lock()?;
//! for frame in 0..100 {
//!     for detector in all_detectors() {
//!         for target in all_stealthed_targets() {
//!             modifier_calc.calculate_detection_modifier(...)?;  // Same calculation!
//!         }
//!     }
//! }
//! ```
//!
//! ## Batch Operation Usage
//!
//! **Problem**: Processing many units individually causes many lock acquisitions.
//!
//! **Solution**: Use batch operations when available:
//! ```rust,ignore
//! // GOOD: Single lock acquisition for batch
//! let mgr = UPGRADE_MANAGER.lock()?;
//! mgr.grant_upgrade_to_player(player_id, "stealth_upgrade")?;
//! drop(mgr);
//!
//! // BAD: Multiple lock acquisitions for same operation
//! for unit in player_units {
//!     UPGRADE_MANAGER.lock()?.grant_upgrade_to_unit(unit.id, "upgrade")?;
//! }
//! ```
//!
//! ---
//!
//! # Common Pitfalls
//!
//! ## 1. Deadlock Risks and How to Avoid Them
//!
//! **Pitfall**: Acquiring multiple locks in nested calls can cause deadlock.
//! ```rust,ignore
//! // DANGEROUS: Lock order inconsistency
//! fn bad_function_1() {
//!     let stealth = STEALTH_MANAGER.lock()?;    // Lock A
//!     let detection = DETECTION_MANAGER.lock()?; // Lock B
//! }
//!
//! fn bad_function_2() {
//!     let detection = DETECTION_MANAGER.lock()?; // Lock B
//!     let stealth = STEALTH_MANAGER.lock()?;    // Lock A
//!     // If bad_function_1 and bad_function_2 run simultaneously,
//!     // they deadlock waiting for each other's locks!
//! }
//! ```
//!
//! **Solution**: Always acquire locks in the same order, or avoid nested locks:
//! ```rust,ignore
//! // GOOD: Single lock acquisition in detection calculation
//! fn detect_with_single_lock() -> Result<bool> {
//!     let detection = DETECTION_MANAGER.lock()?;
//!     // Do all detection work here with one lock
//!     // Don't call other functions that acquire locks
//!     Ok(detected)
//! }
//!
//! // Or: Release first lock before acquiring second
//! fn detect_safely() -> Result<bool> {
//!     let detected = {
//!         let detection = DETECTION_MANAGER.lock()?;
//!         detection.can_detect(...)?
//!     };  // First lock released
//!
//!     // Now safe to acquire other locks
//!     let stealth = STEALTH_MANAGER.lock()?;
//!     stealth.get_stealth_status(...)?;
//!
//!     Ok(detected)
//! }
//! ```
//!
//! ## 2. Performance Gotchas
//!
//! **Pitfall**: Not understanding O(n) vs O(1) operations.
//! ```rust,ignore
//! // SLOW: O(n) operation in tight loop
//! for frame in 0..10000 {
//!     let targets = DETECTION_MANAGER.lock()?.get_all_targets()?; // O(n)
//!     for target in targets {
//!         // Process target
//!     }
//! }
//!
//! // FAST: Get list once, reuse
//! let targets = DETECTION_MANAGER.lock()?.get_all_targets()?;
//! for frame in 0..10000 {
//!     for target in &targets {
//!         // Process target, no lock acquisition
//!     }
//! }
//! ```
//!
//! **Pitfall**: Not releasing locks after getting data.
//! ```rust,ignore
//! // SLOW: Lock held while doing rendering
//! let mgr = STEALTH_MANAGER.lock()?;
//! let statuses = mgr.get_all_statuses()?;
//! render_with_lock_held(&statuses)?;  // Lock held during rendering!
//!
//! // FAST: Release lock immediately after data retrieval
//! let statuses = {
//!     let mgr = STEALTH_MANAGER.lock()?;
//!     mgr.get_all_statuses()?
//! };  // Lock released
//! render_without_lock(&statuses)?;     // No lock contention
//! ```
//!
//! ## 3. Configuration Mistakes
//!
//! **Pitfall**: Setting invalid strength values.
//! ```rust,ignore
//! // WRONG: Value outside 0.0-100.0 range
//! StealthStrength::new(150.0);  // Gets clamped to 100.0!
//!
//! // CORRECT: Use valid values or helper constructors
//! StealthStrength::new(75.5);            // Custom value
//! StealthStrength::standard_cloak();     // 60.0
//! StealthStrength::strong_stealth();     // 90.0
//! ```
//!
//! **Pitfall**: Not registering objects before using them.
//! ```rust,ignore
//! // WRONG: Operation fails with ObjectNotRegistered
//! STEALTH_MANAGER.lock()?.set_stealth_status(unit_id, status, strength)?;
//! // Error: ObjectNotRegistered { object_id: unit_id }
//!
//! // CORRECT: Register first
//! let mgr = STEALTH_MANAGER.lock()?;
//! mgr.register_object(unit_id)?;
//! mgr.set_stealth_status(unit_id, status, strength)?;
//! ```
//!
//! **Pitfall**: Invalid player IDs (must be 0-7).
//! ```rust,ignore
//! // WRONG: Player 8+ doesn't exist
//! STEALTH_MANAGER.lock()?
//!     .get_stealth_status(unit_id, 10)?;  // Error!
//!
//! // CORRECT: Validate player ID before use
//! if player_id < 8 {
//!     STEALTH_MANAGER.lock()?
//!         .get_stealth_status(unit_id, player_id)?;
//! }
//! ```
//!
//! ## 4. Integration Mistakes
//!
//! **Pitfall**: Not checking conditions before allowing stealth.
//! ```rust,ignore
//! // WRONG: Unit can stealth while attacking
//! unit.set_stealthed(true);
//! unit.set_attacking(true);
//! // Stealth should have been broken!
//!
//! // CORRECT: Check conditions first
//! let mgr = CONDITIONS_MANAGER.lock()?;
//! if mgr.can_maintain_stealth(unit_id)? {
//!     unit.set_stealthed(true);
//! }
//! // Now stealth respects conditions
//! ```
//!
//! **Pitfall**: Not updating disguise visibility per frame.
//! ```rust,ignore
//! // WRONG: Disguise transition never advances
//! DISGUISE_MANAGER.lock()?.apply_disguise(unit_id, template, player, 30)?;
//! // Transaction never completes, unit stuck mid-animation
//!
//! // CORRECT: Call update each frame
//! if mgr.is_transitioning(unit_id)? {
//!     mgr.update_disguise_transition(unit_id)?;
//! }
//! // Transition progresses and completes
//! ```
//!
//! **Pitfall**: Forgetting to dispatch detection events.
//! ```rust,ignore
//! // WRONG: Detection happens but no audio/radar feedback
//! DETECTION_MANAGER.lock()?.can_detect(...)?;
//! // Player detects nothing, no radar ping, no audio
//!
//! // CORRECT: Record detection event
//! EVENT_MANAGER.lock()?.record_detection_event(target, detector, player, frame)?;
//! // Event queued for rendering/audio systems to process
//! ```
//!
//! ---
//!
//! # Summary
//!
//! The Stealth & Detection System provides:
//!
//! - **Comprehensive API** for all stealth-related functionality
//! - **Thread-safe operations** with minimal lock contention
//! - **Event-driven architecture** for decoupled subsystems
//! - **Flexible configuration** supporting many game scenarios
//! - **Performance optimizations** for real-time gameplay
//!
//! For detailed implementation, see individual manager modules:
//! - `stealth_manager.rs` - Core stealth state
//! - `detection_manager.rs` - Detection capabilities
//! - `stealth_conditions.rs` - Condition flags
//! - `stealth_upgrade.rs` - Capability management
//! - `stealth_special_power.rs` - Temporary grants
//! - `disguise_manager.rs` - Visual cloaking
//! - `detection_events.rs` - Event dispatching
//! - `detection_modifiers.rs` - Advanced calculations
//! - `stealth_errors.rs` - Error types
//! - `stealth_validation.rs` - Input validation

// This module is pure documentation - no code
// All implementations are in the respective manager modules listed above
