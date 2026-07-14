//! Wave 80: CommandButton residual for host superweapon labels / cursors.
//!
//! Freezes retail `CommandButton.ini` residual fields used by ControlBar
//! superweapon buttons (TextLabel / DescriptLabel / ButtonImage /
//! RadiusCursorType / CursorName / InvalidCursorName / Command button name).
//!
//! Covers the 10 `HostSuperweaponKind` baselines (Daisy/A10/Scud/PUC/Nuke/
//! Anthrax/Spectre/Carpet/Artillery/Cruise). Shortcut variants are residual-
//! named but not claimed as live ControlBar GPU cameos.
//!
//! Fail-closed:
//! - Not full CommandButton INI parse / Science-swap cameo matrix
//! - Not full CursorManager / RadiusCursor GPU draw
//! - Shell `playable_claim` stays false; network deferred

use crate::game_logic::special_power_strikes::HostSuperweaponKind;
use serde::{Deserialize, Serialize};

/// Shared InvalidCursorName residual for location-targeted superweapons.
pub const SUPERWEAPON_INVALID_CURSOR: &str = "GenericInvalid";

/// Retail CommandButton residual fields for one host superweapon kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SuperweaponCommandButtonResidual {
    pub command_name: &'static str,
    pub special_power_template: &'static str,
    pub text_label: &'static str,
    pub descript_label: &'static str,
    pub button_image: &'static str,
    /// RadiusCursorType residual (None when CursorName is used instead).
    pub radius_cursor_type: Option<&'static str>,
    /// CursorName residual (Particle Uplink uses MouseCursor; most use radius).
    pub cursor_name: Option<&'static str>,
    pub shortcut_command_name: &'static str,
    pub shortcut_text_label: &'static str,
}

impl HostSuperweaponKind {
    /// Retail CommandButton residual pack for this host superweapon kind.
    pub fn command_button_residual(self) -> SuperweaponCommandButtonResidual {
        match self {
            HostSuperweaponKind::DaisyCutter => SuperweaponCommandButtonResidual {
                command_name: "Command_DaisyCutter",
                special_power_template: "SuperweaponDaisyCutter",
                text_label: "CONTROLBAR:DaisyCutter",
                descript_label: "CONTROLBAR:TooltipDaisyCutter",
                button_image: "SACDaisyCutter",
                radius_cursor_type: Some("DAISYCUTTER"),
                cursor_name: None,
                shortcut_command_name: "Command_DaisyCutterFromShortcut",
                shortcut_text_label: "OBJECT:DaisyCutterBomb",
            },
            HostSuperweaponKind::A10Strike => SuperweaponCommandButtonResidual {
                command_name: "Command_A10ThunderboltMissileStrike",
                special_power_template: "SuperweaponA10ThunderboltMissileStrike",
                text_label: "CONTROLBAR:A10ThunderboltMissileStrike",
                descript_label: "CONTROLBAR:TooltipA10Strike",
                button_image: "SSA10Attack",
                radius_cursor_type: Some("A10STRIKE"),
                cursor_name: None,
                shortcut_command_name: "Command_A10ThunderboltMissileStrikeFromShortcut",
                shortcut_text_label: "GUI:SuperweaponA10ThunderboltMissileStrike",
            },
            HostSuperweaponKind::ScudStorm => SuperweaponCommandButtonResidual {
                command_name: "Command_ScudStorm",
                special_power_template: "SuperweaponScudStorm",
                text_label: "CONTROLBAR:ScudStorm",
                descript_label: "CONTROLBAR:TooltipFireSCUDStorm",
                button_image: "SSScudStorm",
                radius_cursor_type: Some("SCUDSTORM"),
                cursor_name: None,
                shortcut_command_name: "Command_ScudStormFromShortcut",
                shortcut_text_label: "CONTROLBAR:ScudStormShortcut",
            },
            HostSuperweaponKind::ParticleCannon => SuperweaponCommandButtonResidual {
                command_name: "Command_FireParticleUplinkCannon",
                special_power_template: "SuperweaponParticleUplinkCannon",
                text_label: "CONTROLBAR:FireParticleUplinkCannon",
                descript_label: "CONTROLBAR:TooltipFireParticleUplinkCannon",
                button_image: "SSParticleFire",
                // Retail lists CursorName twice; last wins = ParticleUplinkCannon.
                radius_cursor_type: None,
                cursor_name: Some("ParticleUplinkCannon"),
                shortcut_command_name: "Command_FireParticleUplinkCannonFromShortcut",
                shortcut_text_label: "CONTROLBAR:FireParticleUplinkCannonShortcut",
            },
            HostSuperweaponKind::NuclearMissile => SuperweaponCommandButtonResidual {
                command_name: "Command_NeutronMissile",
                special_power_template: "SuperweaponNeutronMissile",
                text_label: "CONTROLBAR:NeutronMissile",
                descript_label: "CONTROLBAR:TooltipFireNukeMissile",
                button_image: "SNNukeLaunch",
                radius_cursor_type: Some("NUCLEARMISSILE"),
                cursor_name: None,
                shortcut_command_name: "Command_NeutronMissileFromShortcut",
                shortcut_text_label: "CONTROLBAR:NeutronMissileShortcut",
            },
            HostSuperweaponKind::AnthraxBomb => SuperweaponCommandButtonResidual {
                command_name: "Command_AnthraxBomb",
                special_power_template: "SuperweaponAnthraxBomb",
                text_label: "CONTROLBAR:AnthraxBomb",
                descript_label: "CONTROLBAR:TooltipFireAnthraxBomb",
                button_image: "SSAnthraxBomb",
                radius_cursor_type: Some("ANTHRAXBOMB"),
                cursor_name: None,
                shortcut_command_name: "Command_AnthraxBombFromShortcut",
                shortcut_text_label: "OBJECT:AnthraxBomb",
            },
            HostSuperweaponKind::SpectreGunship => SuperweaponCommandButtonResidual {
                command_name: "Command_SpectreGunship",
                special_power_template: "SuperweaponSpectreGunship",
                text_label: "CONTROLBAR:SpectreGunship",
                descript_label: "CONTROLBAR:TooltipSpectreGunship",
                button_image: "SASpGunship",
                radius_cursor_type: Some("SPECTREGUNSHIP"),
                cursor_name: None,
                shortcut_command_name: "Command_SpectreGunshipFromShortcut",
                shortcut_text_label: "CONTROLBAR:SpectreGunshipFromShortcut",
            },
            HostSuperweaponKind::CarpetBomb => SuperweaponCommandButtonResidual {
                command_name: "Command_CarpetBomb",
                special_power_template: "SuperweaponCarpetBomb",
                text_label: "CONTROLBAR:CarpetBomb",
                descript_label: "CONTROLBAR:TooltipCarpetBomb",
                button_image: "SSCarpetBomb",
                radius_cursor_type: Some("CARPETBOMB"),
                cursor_name: None,
                shortcut_command_name: "Command_CarpetBombFromShortcut",
                shortcut_text_label: "OBJECT:CarpetBomb",
            },
            HostSuperweaponKind::ArtilleryBarrage => SuperweaponCommandButtonResidual {
                command_name: "Command_ArtilleryBarrage",
                special_power_template: "SuperweaponArtilleryBarrage",
                text_label: "CONTROLBAR:ArtilleryBarrage",
                descript_label: "CONTROLBAR:TooltipFireArtilleryBarrage",
                button_image: "SSBarrage",
                radius_cursor_type: Some("ARTILLERYBARRAGE"),
                cursor_name: None,
                shortcut_command_name: "Command_ArtilleryBarrageFromShortcut",
                shortcut_text_label: "CONTROLBAR:NoHotKeyArtilleryBarrage",
            },
            HostSuperweaponKind::CruiseMissile => SuperweaponCommandButtonResidual {
                // SupW general residual (SUPR_SPECIAL_CRUISE_MISSILE).
                command_name: "SupW_Command_CruiseMissile",
                special_power_template: "SupW_CruiseMissile",
                text_label: "CONTROLBAR:ICBM",
                descript_label: "CONTROLBAR:TooltipFireNukeMissile",
                button_image: "SNNukeLaunch",
                radius_cursor_type: Some("NUCLEARMISSILE"),
                cursor_name: None,
                shortcut_command_name: "SupW_Command_CruiseMissileFromShortcut",
                shortcut_text_label: "CONTROLBAR:ICBMShortcut",
            },
        }
    }
}

/// All host superweapon kinds for residual pack iteration.
pub const HOST_SUPERWEAPON_COMMAND_BUTTON_KINDS: [HostSuperweaponKind; 10] = [
    HostSuperweaponKind::DaisyCutter,
    HostSuperweaponKind::A10Strike,
    HostSuperweaponKind::ScudStorm,
    HostSuperweaponKind::ParticleCannon,
    HostSuperweaponKind::NuclearMissile,
    HostSuperweaponKind::AnthraxBomb,
    HostSuperweaponKind::SpectreGunship,
    HostSuperweaponKind::CarpetBomb,
    HostSuperweaponKind::ArtilleryBarrage,
    HostSuperweaponKind::CruiseMissile,
];

/// Wave 80 honesty: superweapon CommandButton label/cursor residual pack.
///
/// Fail-closed: not full CommandButton INI parse / CursorManager GPU path.
pub fn honesty_command_button_superweapon_residual_pack_wave80() -> bool {
    let daisy = HostSuperweaponKind::DaisyCutter.command_button_residual();
    let a10 = HostSuperweaponKind::A10Strike.command_button_residual();
    let scud = HostSuperweaponKind::ScudStorm.command_button_residual();
    let puc = HostSuperweaponKind::ParticleCannon.command_button_residual();
    let nuke = HostSuperweaponKind::NuclearMissile.command_button_residual();
    let anthrax = HostSuperweaponKind::AnthraxBomb.command_button_residual();
    let spectre = HostSuperweaponKind::SpectreGunship.command_button_residual();
    let carpet = HostSuperweaponKind::CarpetBomb.command_button_residual();
    let arty = HostSuperweaponKind::ArtilleryBarrage.command_button_residual();
    let cruise = HostSuperweaponKind::CruiseMissile.command_button_residual();

    HOST_SUPERWEAPON_COMMAND_BUTTON_KINDS.len() == 10
        && SUPERWEAPON_INVALID_CURSOR == "GenericInvalid"
        // Daisy
        && daisy.command_name == "Command_DaisyCutter"
        && daisy.text_label == "CONTROLBAR:DaisyCutter"
        && daisy.descript_label == "CONTROLBAR:TooltipDaisyCutter"
        && daisy.button_image == "SACDaisyCutter"
        && daisy.radius_cursor_type == Some("DAISYCUTTER")
        && daisy.cursor_name.is_none()
        && daisy.shortcut_text_label == "OBJECT:DaisyCutterBomb"
        // A10
        && a10.command_name == "Command_A10ThunderboltMissileStrike"
        && a10.radius_cursor_type == Some("A10STRIKE")
        && a10.button_image == "SSA10Attack"
        // Scud
        && scud.command_name == "Command_ScudStorm"
        && scud.radius_cursor_type == Some("SCUDSTORM")
        && scud.descript_label == "CONTROLBAR:TooltipFireSCUDStorm"
        // Particle Uplink: CursorName residual (no RadiusCursorType).
        && puc.command_name == "Command_FireParticleUplinkCannon"
        && puc.cursor_name == Some("ParticleUplinkCannon")
        && puc.radius_cursor_type.is_none()
        && puc.button_image == "SSParticleFire"
        // Nuke / Neutron
        && nuke.command_name == "Command_NeutronMissile"
        && nuke.special_power_template == "SuperweaponNeutronMissile"
        && nuke.radius_cursor_type == Some("NUCLEARMISSILE")
        && nuke.button_image == "SNNukeLaunch"
        // Anthrax
        && anthrax.radius_cursor_type == Some("ANTHRAXBOMB")
        && anthrax.button_image == "SSAnthraxBomb"
        // Spectre
        && spectre.radius_cursor_type == Some("SPECTREGUNSHIP")
        && spectre.button_image == "SASpGunship"
        // Carpet
        && carpet.radius_cursor_type == Some("CARPETBOMB")
        && carpet.button_image == "SSCarpetBomb"
        // Artillery
        && arty.radius_cursor_type == Some("ARTILLERYBARRAGE")
        && arty.button_image == "SSBarrage"
        && arty.shortcut_text_label == "CONTROLBAR:NoHotKeyArtilleryBarrage"
        // Cruise (SupW ICBM residual)
        && cruise.command_name == "SupW_Command_CruiseMissile"
        && cruise.special_power_template == "SupW_CruiseMissile"
        && cruise.text_label == "CONTROLBAR:ICBM"
        && cruise.radius_cursor_type == Some("NUCLEARMISSILE")
        // Every kind has non-empty labels + either radius or cursor residual.
        && HOST_SUPERWEAPON_COMMAND_BUTTON_KINDS.iter().all(|k| {
            let b = k.command_button_residual();
            !b.command_name.is_empty()
                && !b.text_label.is_empty()
                && !b.descript_label.is_empty()
                && !b.button_image.is_empty()
                && !b.special_power_template.is_empty()
                && (b.radius_cursor_type.is_some() || b.cursor_name.is_some())
                && !b.shortcut_command_name.is_empty()
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_button_superweapon_residual_pack_wave80_honesty() {
        assert!(honesty_command_button_superweapon_residual_pack_wave80());
        let daisy = HostSuperweaponKind::DaisyCutter.command_button_residual();
        assert_eq!(daisy.radius_cursor_type, Some("DAISYCUTTER"));
        let puc = HostSuperweaponKind::ParticleCannon.command_button_residual();
        assert_eq!(puc.cursor_name, Some("ParticleUplinkCannon"));
    }
}
