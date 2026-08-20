// C++ InGameUI::s_fieldParseTable *RadiusCursor keys (InGameUI.cpp:820-853).
// Included by ingame_ui/mod.rs.

impl RadiusCursorType {
    /// INI token for this cursor's RadiusDecalTemplate block.
    pub fn ini_token(self) -> Option<&'static str> {
        Some(match self {
            Self::None | Self::Count => return None,
            Self::AttackDamageArea => "AttackDamageAreaRadiusCursor",
            Self::AttackScatterArea => "AttackScatterAreaRadiusCursor",
            Self::AttackContinueArea => "AttackContinueAreaRadiusCursor",
            Self::FriendlySpecialPower => "FriendlySpecialPowerRadiusCursor",
            Self::OffensiveSpecialPower => "OffensiveSpecialPowerRadiusCursor",
            Self::SuperweaponScatterArea => "SuperweaponScatterAreaRadiusCursor",
            Self::GuardArea => "GuardAreaRadiusCursor",
            Self::EmergencyRepair => "EmergencyRepairRadiusCursor",
            Self::ParticleCannon => "ParticleCannonRadiusCursor",
            Self::A10Strike => "A10StrikeRadiusCursor",
            Self::CarpetBomb => "CarpetBombRadiusCursor",
            Self::DaisyCutter => "DaisyCutterRadiusCursor",
            Self::Paradrop => "ParadropRadiusCursor",
            Self::SpySatellite => "SpySatelliteRadiusCursor",
            Self::SpectreGunship => "SpectreGunshipRadiusCursor",
            Self::HelixNapalmBomb => "HelixNapalmBombRadiusCursor",
            Self::NuclearMissile => "NuclearMissileRadiusCursor",
            Self::EmpPulse => "EMPPulseRadiusCursor",
            Self::ArtilleryBarrage => "ArtilleryRadiusCursor",
            Self::Frenzy => "FrenzyRadiusCursor",
            Self::NapalmStrike => "NapalmStrikeRadiusCursor",
            Self::ClusterMines => "ClusterMinesRadiusCursor",
            Self::ScudStorm => "ScudStormRadiusCursor",
            Self::AnthraxBomb => "AnthraxBombRadiusCursor",
            Self::Ambush => "AmbushRadiusCursor",
            Self::Radar => "RadarRadiusCursor",
            Self::SpyDrone => "SpyDroneRadiusCursor",
            Self::ClearMines => "ClearMinesRadiusCursor",
            Self::Ambulance => "AmbulanceRadiusCursor",
        })
    }
}

impl InGameUI {
    /// Authored InGameUI.ini textures (C++ m_radiusCursors[RADIUSCURSOR_*]).
    /// Distinct from the generic SCCAttackDamageArea fallback.
    fn authored_radius_cursor_texture(cursor_type: RadiusCursorType) -> &'static str {
        match cursor_type {
            RadiusCursorType::None | RadiusCursorType::Count => "",
            RadiusCursorType::AttackDamageArea => "SCCAttackDamageArea",
            RadiusCursorType::AttackScatterArea => "SCCAttackScatterArea",
            RadiusCursorType::AttackContinueArea => "SCCAttackContinueArea",
            RadiusCursorType::FriendlySpecialPower => "SCCSpecialPowerFriendly",
            RadiusCursorType::OffensiveSpecialPower => "SCCSpecialPowerOffensive",
            RadiusCursorType::SuperweaponScatterArea => "SCCSuperweaponScatter",
            RadiusCursorType::GuardArea => "SCCGuard",
            RadiusCursorType::EmergencyRepair => "SCCRepair",
            RadiusCursorType::ParticleCannon => "EXParticleCannon",
            RadiusCursorType::A10Strike => "EXA10Strike",
            RadiusCursorType::CarpetBomb => "EXCarpetBomb",
            RadiusCursorType::DaisyCutter => "EXDaisyCutter",
            RadiusCursorType::Paradrop => "EXParadrop",
            RadiusCursorType::SpySatellite => "EXSpySatellite",
            RadiusCursorType::SpectreGunship => "EXSpectreGunship",
            RadiusCursorType::HelixNapalmBomb => "EXHelixNapalm",
            RadiusCursorType::NuclearMissile => "EXNuke",
            RadiusCursorType::EmpPulse => "EXEmpPulse",
            RadiusCursorType::ArtilleryBarrage => "EXArtillery",
            RadiusCursorType::Frenzy => "SCCFrenzy",
            RadiusCursorType::NapalmStrike => "EXNapalmStrike",
            RadiusCursorType::ClusterMines => "EXClusterMines",
            RadiusCursorType::ScudStorm => "EXScudStorm",
            RadiusCursorType::AnthraxBomb => "EXAnthraxBomb",
            RadiusCursorType::Ambush => "SCCAmbush",
            RadiusCursorType::Radar => "SCCRadar",
            RadiusCursorType::SpyDrone => "EXSpyDrone",
            RadiusCursorType::ClearMines => "SCCClearMines",
            RadiusCursorType::Ambulance => "SCCAmbulance",
        }
    }

    fn radius_cursor_templates_from_ini() -> Vec<crate::radius_decal::RadiusDecalTemplate> {
        let mut templates = Vec::with_capacity(RadiusCursorType::COUNT as usize);
        for index in 0..RadiusCursorType::COUNT as u32 {
            let cursor = match index {
                0 => RadiusCursorType::None,
                1 => RadiusCursorType::AttackDamageArea,
                2 => RadiusCursorType::AttackScatterArea,
                3 => RadiusCursorType::AttackContinueArea,
                4 => RadiusCursorType::GuardArea,
                5 => RadiusCursorType::EmergencyRepair,
                6 => RadiusCursorType::FriendlySpecialPower,
                7 => RadiusCursorType::OffensiveSpecialPower,
                8 => RadiusCursorType::SuperweaponScatterArea,
                9 => RadiusCursorType::ParticleCannon,
                10 => RadiusCursorType::A10Strike,
                11 => RadiusCursorType::CarpetBomb,
                12 => RadiusCursorType::DaisyCutter,
                13 => RadiusCursorType::Paradrop,
                14 => RadiusCursorType::SpySatellite,
                15 => RadiusCursorType::SpectreGunship,
                16 => RadiusCursorType::HelixNapalmBomb,
                17 => RadiusCursorType::NuclearMissile,
                18 => RadiusCursorType::EmpPulse,
                19 => RadiusCursorType::ArtilleryBarrage,
                20 => RadiusCursorType::NapalmStrike,
                21 => RadiusCursorType::ClusterMines,
                22 => RadiusCursorType::ScudStorm,
                23 => RadiusCursorType::AnthraxBomb,
                24 => RadiusCursorType::Ambush,
                25 => RadiusCursorType::Radar,
                26 => RadiusCursorType::SpyDrone,
                27 => RadiusCursorType::Frenzy,
                28 => RadiusCursorType::ClearMines,
                29 => RadiusCursorType::Ambulance,
                _ => RadiusCursorType::None,
            };
            let texture = Self::authored_radius_cursor_texture(cursor);
            templates.push(if texture.is_empty() {
                crate::radius_decal::RadiusDecalTemplate::default()
            } else {
                crate::radius_decal::RadiusDecalTemplate::with_texture(texture)
            });
        }
        templates
    }
}
