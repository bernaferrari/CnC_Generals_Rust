//! Audio Event Dispatcher
//!
//! Connects game events to audio playback
//! Integrates with game systems to trigger appropriate sounds
//!
//! Matches C++ event handling patterns from:
//! /GeneralsMD/Code/GameEngine/Source/GameClient/

use std::sync::Arc;
use crate::audio_manager::{GameAudioManager, AudioPosition, AudioPriority};

/// Unit audio events
#[derive(Debug, Clone)]
pub enum UnitAudioEvent {
    /// Unit selected by player
    Selected { unit_type: String },
    /// Unit moved to new location
    Moved { unit_type: String },
    /// Unit attacked target
    Attacked { unit_type: String, position: AudioPosition },
    /// Unit took damage
    Damaged { unit_type: String, position: AudioPosition },
    /// Unit died/destroyed
    Died { unit_type: String, position: AudioPosition },
    /// Unit created/spawned
    Created { unit_type: String, position: AudioPosition },
    /// Unit entered vehicle/building
    Entered { unit_type: String },
    /// Unit exited vehicle/building
    Exited { unit_type: String },
    /// Unit promoted/leveled up
    Promoted { unit_type: String },
    /// Unit ability activated
    AbilityActivated { unit_type: String, ability: String },
}

/// Building audio events
#[derive(Debug, Clone)]
pub enum BuildingAudioEvent {
    /// Building placement started
    PlacementStarted { building_type: String },
    /// Building construction began
    ConstructionStarted { building_type: String, position: AudioPosition },
    /// Building construction progressed
    ConstructionProgress { building_type: String, percent: f32 },
    /// Building construction completed
    ConstructionCompleted { building_type: String, position: AudioPosition },
    /// Building sold
    Sold { building_type: String, position: AudioPosition },
    /// Building destroyed
    Destroyed { building_type: String, position: AudioPosition },
    /// Building power state changed
    PowerChanged { building_type: String, powered: bool },
    /// Building captured
    Captured { building_type: String, position: AudioPosition },
    /// Building ability activated
    AbilityActivated { building_type: String, ability: String, position: AudioPosition },
}

/// Weapon audio events
#[derive(Debug, Clone)]
pub enum WeaponAudioEvent {
    /// Weapon fired
    Fired { weapon_type: String, position: AudioPosition },
    /// Projectile hit target
    Hit { weapon_type: String, position: AudioPosition },
    /// Projectile missed
    Miss { weapon_type: String, position: AudioPosition },
    /// Weapon reloading
    Reloading { weapon_type: String },
}

/// UI audio events
#[derive(Debug, Clone)]
pub enum UIAudioEvent {
    /// Mouse clicked on UI element
    Click,
    /// Mouse hovered over UI element
    Hover,
    /// Button pressed
    ButtonPress { button_type: String },
    /// Menu opened
    MenuOpen,
    /// Menu closed
    MenuClose,
    /// Error/invalid action
    Error,
    /// Warning message
    Warning,
    /// Notification received
    Notification { notification_type: String },
    /// Money collected
    MoneyCollected { amount: i32 },
    /// Insufficient funds
    InsufficientFunds,
    /// Cannot build here
    CannotBuild,
    /// Unit ready
    UnitReady { unit_type: String },
    /// Building ready
    BuildingReady { building_type: String },
    /// Upgrade complete
    UpgradeComplete { upgrade_type: String },
    /// Research complete
    ResearchComplete { research_type: String },
    /// Low power
    LowPower,
    /// Base under attack
    BaseUnderAttack,
    /// Enemy detected
    EnemyDetected,
}

/// Audio event dispatcher
///
/// Central system for routing game events to audio playback
pub struct AudioEventDispatcher {
    audio_manager: Arc<GameAudioManager>,
}

impl AudioEventDispatcher {
    /// Create new audio event dispatcher
    pub fn new(audio_manager: Arc<GameAudioManager>) -> Self {
        Self {
            audio_manager,
        }
    }

    /// Dispatch unit audio event
    pub fn dispatch_unit_event(&self, event: UnitAudioEvent) {
        match event {
            UnitAudioEvent::Selected { ref unit_type } => {
                let sound_path = format!("Sounds/Units/{}/Select.wav", unit_type);
                let _ = self.audio_manager.play_sound_2d(
                    sound_path,
                    AudioPriority::NORMAL,
                    1.0,
                    false,
                );
            }

            UnitAudioEvent::Moved { ref unit_type } => {
                let sound_path = format!("Sounds/Units/{}/Move.wav", unit_type);
                let _ = self.audio_manager.play_sound_2d(
                    sound_path,
                    AudioPriority::NORMAL,
                    0.8,
                    false,
                );
            }

            UnitAudioEvent::Attacked { ref unit_type, position } => {
                let sound_path = format!("Sounds/Units/{}/Attack.wav", unit_type);
                let _ = self.audio_manager.play_sound_3d(
                    sound_path,
                    position,
                    crate::audio_manager::AudioVelocity::zero(),
                    crate::audio_manager::AudioAttenuation::default(),
                    AudioPriority::HIGH,
                    1.0,
                    false,
                );
            }

            UnitAudioEvent::Damaged { ref unit_type, position } => {
                let sound_path = format!("Sounds/Units/{}/Damage.wav", unit_type);
                let _ = self.audio_manager.play_sound_3d(
                    sound_path,
                    position,
                    crate::audio_manager::AudioVelocity::zero(),
                    crate::audio_manager::AudioAttenuation::default(),
                    AudioPriority::HIGH,
                    1.0,
                    false,
                );
            }

            UnitAudioEvent::Died { ref unit_type, position } => {
                let sound_path = format!("Sounds/Units/{}/Die.wav", unit_type);
                let _ = self.audio_manager.play_sound_3d(
                    sound_path,
                    position,
                    crate::audio_manager::AudioVelocity::zero(),
                    crate::audio_manager::AudioAttenuation::default(),
                    AudioPriority::HIGH,
                    1.0,
                    false,
                );
            }

            UnitAudioEvent::Created { ref unit_type, position } => {
                let sound_path = format!("Sounds/Units/{}/Create.wav", unit_type);
                let _ = self.audio_manager.play_sound_3d(
                    sound_path,
                    position,
                    crate::audio_manager::AudioVelocity::zero(),
                    crate::audio_manager::AudioAttenuation::default(),
                    AudioPriority::NORMAL,
                    0.8,
                    false,
                );
            }

            UnitAudioEvent::Promoted { ref unit_type } => {
                let sound_path = format!("Sounds/Units/{}/Promoted.wav", unit_type);
                let _ = self.audio_manager.play_sound_2d(
                    sound_path,
                    AudioPriority::HIGH,
                    1.0,
                    false,
                );
            }

            UnitAudioEvent::AbilityActivated { ref unit_type, ref ability } => {
                let sound_path = format!("Sounds/Units/{}/{}.wav", unit_type, ability);
                let _ = self.audio_manager.play_sound_2d(
                    sound_path,
                    AudioPriority::HIGH,
                    1.0,
                    false,
                );
            }

            _ => {
                // Handle remaining variants
            }
        }
    }

    /// Dispatch building audio event
    pub fn dispatch_building_event(&self, event: BuildingAudioEvent) {
        match event {
            BuildingAudioEvent::PlacementStarted { ref building_type } => {
                let sound_path = format!("Sounds/Buildings/{}/Placement.wav", building_type);
                let _ = self.audio_manager.play_sound_2d(
                    sound_path,
                    AudioPriority::NORMAL,
                    0.7,
                    false,
                );
            }

            BuildingAudioEvent::ConstructionStarted { ref building_type, position } => {
                let sound_path = format!("Sounds/Buildings/{}/Construction.wav", building_type);
                let _ = self.audio_manager.play_sound_3d(
                    sound_path,
                    position,
                    crate::audio_manager::AudioVelocity::zero(),
                    crate::audio_manager::AudioAttenuation::default(),
                    AudioPriority::NORMAL,
                    0.6,
                    true, // Loop construction sound
                );
            }

            BuildingAudioEvent::ConstructionCompleted { ref building_type, position } => {
                let sound_path = format!("Sounds/Buildings/{}/Complete.wav", building_type);
                let _ = self.audio_manager.play_sound_3d(
                    sound_path,
                    position,
                    crate::audio_manager::AudioVelocity::zero(),
                    crate::audio_manager::AudioAttenuation::default(),
                    AudioPriority::HIGH,
                    1.0,
                    false,
                );

                // Queue voice notification
                self.audio_manager.queue_speech(
                    "Sounds/EVA/ConstructionComplete.wav",
                    AudioPriority::HIGH,
                    None,
                );
            }

            BuildingAudioEvent::Destroyed { ref building_type, position } => {
                let sound_path = format!("Sounds/Buildings/{}/Destroy.wav", building_type);
                let _ = self.audio_manager.play_sound_3d(
                    sound_path,
                    position,
                    crate::audio_manager::AudioVelocity::zero(),
                    crate::audio_manager::AudioAttenuation {
                        max_vol_radius: 20.0,
                        dropoff_radius: 200.0,
                    },
                    AudioPriority::HIGHEST,
                    1.0,
                    false,
                );
            }

            BuildingAudioEvent::PowerChanged { ref building_type, powered } => {
                let sound_name = if powered { "PowerOn" } else { "PowerOff" };
                let sound_path = format!("Sounds/Buildings/{}/{}.wav", building_type, sound_name);
                let _ = self.audio_manager.play_sound_2d(
                    sound_path,
                    AudioPriority::NORMAL,
                    0.8,
                    false,
                );
            }

            _ => {
                // Handle remaining variants
            }
        }
    }

    /// Dispatch weapon audio event
    pub fn dispatch_weapon_event(&self, event: WeaponAudioEvent) {
        match event {
            WeaponAudioEvent::Fired { ref weapon_type, position } => {
                let sound_path = format!("Sounds/Weapons/{}/Fire.wav", weapon_type);
                let _ = self.audio_manager.play_sound_3d(
                    sound_path,
                    position,
                    crate::audio_manager::AudioVelocity::zero(),
                    crate::audio_manager::AudioAttenuation {
                        max_vol_radius: 5.0,
                        dropoff_radius: 100.0,
                    },
                    AudioPriority::HIGH,
                    1.0,
                    false,
                );
            }

            WeaponAudioEvent::Hit { ref weapon_type, position } => {
                let sound_path = format!("Sounds/Weapons/{}/Hit.wav", weapon_type);
                let _ = self.audio_manager.play_sound_3d(
                    sound_path,
                    position,
                    crate::audio_manager::AudioVelocity::zero(),
                    crate::audio_manager::AudioAttenuation::default(),
                    AudioPriority::HIGH,
                    0.9,
                    false,
                );
            }

            _ => {
                // Handle remaining variants
            }
        }
    }

    /// Dispatch UI audio event
    pub fn dispatch_ui_event(&self, event: UIAudioEvent) {
        match event {
            UIAudioEvent::Click => {
                let _ = self.audio_manager.play_sound_2d(
                    "Sounds/UI/Click.wav",
                    AudioPriority::NORMAL,
                    0.7,
                    false,
                );
            }

            UIAudioEvent::Hover => {
                let _ = self.audio_manager.play_sound_2d(
                    "Sounds/UI/Hover.wav",
                    AudioPriority::LOW,
                    0.5,
                    false,
                );
            }

            UIAudioEvent::ButtonPress { ref button_type } => {
                let sound_path = format!("Sounds/UI/{}.wav", button_type);
                let _ = self.audio_manager.play_sound_2d(
                    sound_path,
                    AudioPriority::NORMAL,
                    0.8,
                    false,
                );
            }

            UIAudioEvent::MenuOpen => {
                let _ = self.audio_manager.play_sound_2d(
                    "Sounds/UI/MenuOpen.wav",
                    AudioPriority::NORMAL,
                    0.7,
                    false,
                );
            }

            UIAudioEvent::MenuClose => {
                let _ = self.audio_manager.play_sound_2d(
                    "Sounds/UI/MenuClose.wav",
                    AudioPriority::NORMAL,
                    0.7,
                    false,
                );
            }

            UIAudioEvent::Error => {
                let _ = self.audio_manager.play_sound_2d(
                    "Sounds/UI/Error.wav",
                    AudioPriority::HIGH,
                    0.9,
                    false,
                );
            }

            UIAudioEvent::Warning => {
                let _ = self.audio_manager.play_sound_2d(
                    "Sounds/UI/Warning.wav",
                    AudioPriority::HIGH,
                    0.9,
                    false,
                );
            }

            UIAudioEvent::MoneyCollected { amount } => {
                let _ = self.audio_manager.play_sound_2d(
                    "Sounds/UI/MoneyCollected.wav",
                    AudioPriority::NORMAL,
                    0.8,
                    false,
                );
            }

            UIAudioEvent::InsufficientFunds => {
                let _ = self.audio_manager.play_sound_2d(
                    "Sounds/UI/InsufficientFunds.wav",
                    AudioPriority::HIGH,
                    1.0,
                    false,
                );

                self.audio_manager.queue_speech(
                    "Sounds/EVA/InsufficientFunds.wav",
                    AudioPriority::HIGH,
                    None,
                );
            }

            UIAudioEvent::BaseUnderAttack => {
                let _ = self.audio_manager.play_sound_2d(
                    "Sounds/UI/BaseUnderAttack.wav",
                    AudioPriority::HIGHEST,
                    1.0,
                    false,
                );

                self.audio_manager.queue_speech(
                    "Sounds/EVA/BaseUnderAttack.wav",
                    AudioPriority::HIGHEST,
                    None,
                );
            }

            UIAudioEvent::UnitReady { ref unit_type } => {
                self.audio_manager.queue_speech(
                    format!("Sounds/EVA/UnitReady/{}.wav", unit_type),
                    AudioPriority::HIGH,
                    None,
                );
            }

            UIAudioEvent::BuildingReady { ref building_type } => {
                self.audio_manager.queue_speech(
                    format!("Sounds/EVA/BuildingReady/{}.wav", building_type),
                    AudioPriority::HIGH,
                    None,
                );
            }

            _ => {
                // Handle remaining variants
            }
        }
    }

    /// Get audio manager reference
    pub fn get_audio_manager(&self) -> &Arc<GameAudioManager> {
        &self.audio_manager
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unit_event_creation() {
        let event = UnitAudioEvent::Selected {
            unit_type: "Tank".to_string(),
        };

        match event {
            UnitAudioEvent::Selected { unit_type } => {
                assert_eq!(unit_type, "Tank");
            }
            _ => panic!("Wrong event type"),
        }
    }

    #[test]
    fn test_building_event_creation() {
        let event = BuildingAudioEvent::ConstructionCompleted {
            building_type: "PowerPlant".to_string(),
            position: AudioPosition::new(100.0, 0.0, 200.0),
        };

        match event {
            BuildingAudioEvent::ConstructionCompleted { building_type, position } => {
                assert_eq!(building_type, "PowerPlant");
                assert_eq!(position.x, 100.0);
                assert_eq!(position.z, 200.0);
            }
            _ => panic!("Wrong event type"),
        }
    }
}
