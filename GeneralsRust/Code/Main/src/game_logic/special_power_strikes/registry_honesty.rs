//! HostSpecialPowerStrikeRegistry residual honesty methods.
use super::types::*;
use super::*;
impl HostSpecialPowerStrikeRegistry {
    pub fn honesty_howitzer_ok(&self) -> bool {
        self.orbit_fields.iter().any(|f| f.howitzer_ticks > 0)
            || self
                .orbit_fields
                .iter()
                .any(|f| f.damage_applications > 0 && f.howitzer_ticks > 0)
    }

    /// Residual honesty: at least one gattling strafe tick applied.
    pub fn honesty_gattling_ok(&self) -> bool {
        self.orbit_fields.iter().any(|f| f.gattling_ticks > 0)
    }

    /// Residual honesty: gattling continuous-fire ramp reached MEAN or FAST.
    pub fn honesty_gattling_continuous_fire_ok(&self) -> bool {
        self.orbit_fields.iter().any(|f| f.gattling_fire_level >= 1)
    }

    /// Residual honesty: ContinuousFire WeaponBonus ROF residual applications.
    ///
    /// MEAN (200%) and FAST (300%) application counters must have been recorded
    /// on at least one orbit field. Fail-closed: not full WeaponBonusConditionFlags.
    pub fn honesty_gattling_weapon_bonus_rof_ok(&self) -> bool {
        honesty_gattling_weapon_bonus_rof()
            && self.orbit_fields.iter().any(|f| {
                f.gattling_rof_mean_applications > 0 && f.gattling_rof_fast_applications > 0
            })
    }

    /// Residual honesty: CarpetBomb residual pack (Wave 56) constants + applications.
    pub fn honesty_carpet_bomb_residual_pack_ok(&self) -> bool {
        if !honesty_carpet_bomb_residual_pack() {
            return false;
        }
        // Constants-only path when no carpet strike queued.
        if !self
            .strikes
            .values()
            .any(|s| s.kind == HostSuperweaponKind::CarpetBomb)
        {
            return true;
        }
        self.strikes.values().any(|s| {
            s.kind == HostSuperweaponKind::CarpetBomb
                && s.carpet_residual_pack_armed >= 1
                && s.carpet_preferred_height_applications >= 1
                && s.carpet_drop_delay_applications >= 1
                && s.carpet_drop_variance_applications >= 1
                && s.carpet_bomb_count_applications >= 1
                && s.carpet_delivery_distance_applications >= 1
        })
    }

    /// Residual honesty: CruiseMissile/MOAB residual pack (Wave 56).
    pub fn honesty_cruise_missile_residual_pack_ok(&self) -> bool {
        if !honesty_cruise_missile_residual_pack() {
            return false;
        }
        if !self
            .strikes
            .values()
            .any(|s| s.kind == HostSuperweaponKind::CruiseMissile)
        {
            return true;
        }
        self.strikes.values().any(|s| {
            s.kind == HostSuperweaponKind::CruiseMissile
                && s.cruise_residual_pack_armed >= 1
                && s.cruise_loft_applications >= 1
                && s.cruise_height_die_applications >= 1
                && s.cruise_projectile_applications >= 1
                && s.cruise_moab_weapon_applications >= 1
        })
    }

    /// Residual honesty: ArtilleryBarrage residual pack (Wave 56).
    pub fn honesty_artillery_barrage_residual_pack_ok(&self) -> bool {
        if !honesty_artillery_barrage_residual_pack() {
            return false;
        }
        if !self
            .strikes
            .values()
            .any(|s| s.kind == HostSuperweaponKind::ArtilleryBarrage)
        {
            return true;
        }
        self.strikes.values().any(|s| {
            s.kind == HostSuperweaponKind::ArtilleryBarrage
                && s.artillery_residual_pack_armed >= 1
                && s.artillery_cannon_transport_applications >= 1
                && s.artillery_formation_size_applications >= 1
                && s.artillery_delay_delivery_applications >= 1
                && s.artillery_weapon_error_radius_applications >= 1
                && s.artillery_preferred_height_applications >= 1
        })
    }

    /// Residual honesty: Nuke radiation residual pack (Wave 56).
    pub fn honesty_nuke_radiation_residual_pack_ok(&self) -> bool {
        if !honesty_nuke_radiation_residual_pack() {
            return false;
        }
        if self.radiation_fields.is_empty() {
            return true;
        }
        self.radiation_fields.iter().any(|f| {
            f.radiation_residual_pack_armed >= 1
                && f.radiation_suspend_fx_applications >= 1
                && f.radiation_fire_fx_applications >= 1
        })
    }

    /// Residual honesty: Anthrax toxin residual pack (Wave 56).
    pub fn honesty_anthrax_toxin_residual_pack_ok(&self) -> bool {
        if !honesty_anthrax_toxin_residual_pack() {
            return false;
        }
        // Constants-only path when no anthrax toxin field armed.
        let anthrax_fields: Vec<_> = self
            .toxin_fields
            .iter()
            .filter(|f| {
                (f.damage_per_tick - ANTHRAX_TOXIN_DAMAGE_PER_TICK).abs() < 0.01
                    && (f.radius - ANTHRAX_TOXIN_RADIUS).abs() < 0.1
            })
            .collect();
        if anthrax_fields.is_empty() {
            return true;
        }
        anthrax_fields.iter().any(|f| {
            f.toxin_residual_pack_armed >= 1
                && f.toxin_fire_fx_applications >= 1
                && f.toxin_damage_type_applications >= 1
        })
    }

    /// Residual honesty: howitzer continuous-fire ramp reached MEAN or FAST.
    pub fn honesty_howitzer_continuous_fire_ok(&self) -> bool {
        self.orbit_fields.iter().any(|f| f.howitzer_fire_level >= 1)
    }

    /// Residual honesty: ContinuousFireCoast cool-down applied at least once.
    pub fn honesty_continuous_fire_coast_ok(&self) -> bool {
        self.orbit_fields
            .iter()
            .any(|f| f.gattling_coast_applications > 0 || f.howitzer_coast_applications > 0)
            && SPECTRE_CONTINUOUS_FIRE_COAST_FRAMES == 60
    }

    /// Residual honesty: VoiceRapidFire cue when ContinuousFire entered FAST.
    pub fn honesty_voice_rapid_fire_ok(&self) -> bool {
        self.orbit_fields
            .iter()
            .any(|f| f.rapid_fire_voice_cues > 0)
            && SPECTRE_VOICE_RAPID_FIRE_AUDIO.contains("Rapid")
    }

    /// Residual honesty: SpectreHowitzerShell projectile residual spawned.
    ///
    /// Fail-closed: not full DumbProjectileBehavior Object / HeightDie flight /
    /// live W3D GPU shell drawable / PhysicsBehavior mass path.
    pub fn honesty_howitzer_shell_ok(&self) -> bool {
        self.orbit_fields.iter().any(|f| {
            f.howitzer_shells_spawned > 0
                && f.howitzer_shell_fire_fx >= f.howitzer_shells_spawned
                && f.howitzer_shell_detonation_fx >= f.howitzer_shells_spawned
                && f.howitzer_shell_height_die_delays >= f.howitzer_shells_spawned
                && f.howitzer_shell_fire_sounds >= f.howitzer_shells_spawned
                && f.howitzer_shell_dumb_projectile_applications >= f.howitzer_shells_spawned
                && f.howitzer_shell_physics_mass_applications >= f.howitzer_shells_spawned
                && f.howitzer_shell_death_detonated_applications >= f.howitzer_shells_spawned
                && f.howitzer_shell_only_moving_down_applications >= f.howitzer_shells_spawned
                && f.howitzer_shell_model_draw_applications >= f.howitzer_shells_spawned
                && f.howitzer_shell_scale_applications >= f.howitzer_shells_spawned
                && f.howitzer_shell_shadow_applications >= f.howitzer_shells_spawned
                && f.howitzer_shell_geometry_applications >= f.howitzer_shells_spawned
                && f.howitzer_shell_max_health_applications >= f.howitzer_shells_spawned
        }) && SPECTRE_HOWITZER_SHELL_OBJECT == "SpectreHowitzerShell"
            && SPECTRE_HOWITZER_HEIGHT_DIE_INITIAL_DELAY_FRAMES == 30
            && (SPECTRE_HOWITZER_WEAPON_SPEED - 999.0).abs() < 0.01
            && SPECTRE_HOWITZER_FIRE_FX.contains("TankGun")
            && SPECTRE_HOWITZER_DETONATION_FX.contains("SpectreHowitzer")
            && SPECTRE_HOWITZER_FIRE_SOUND.contains("Artillery")
            && (SPECTRE_HOWITZER_HEIGHT_DIE_TARGET_HEIGHT - 1.0).abs() < 0.01
            && (SPECTRE_HOWITZER_SHELL_SCALE - 0.6).abs() < 0.01
            && (SPECTRE_HOWITZER_SHELL_MASS - 1.0).abs() < 0.01
            && SPECTRE_HOWITZER_SHELL_HEIGHT_DIE_ONLY_MOVING_DOWN
            && SPECTRE_HOWITZER_SHELL_MODEL.contains("SpectreShell")
            && SPECTRE_HOWITZER_SHELL_DEATH_DETONATED_FX.contains("NukeGLA")
            && SPECTRE_HOWITZER_SHELL_DEATH_LASERED_OCL.contains("Disintegrate")
            && (SPECTRE_HOWITZER_SHELL_MAX_HEALTH - 100.0).abs() < 0.01
            && SPECTRE_HOWITZER_SHELL_GEOMETRY_IS_SMALL
            && SPECTRE_HOWITZER_SHELL_SHADOW.contains("SHADOW_DECAL")
            && SPECTRE_HOWITZER_SHELL_GEOMETRY == "Cylinder"
    }

    /// Residual honesty: SpectreHowitzerShell DumbProjectileBehavior path residual.
    ///
    /// Fail-closed: not full ThingFactory Object / live W3D GPU ModelDraw / Physics.
    pub fn honesty_howitzer_shell_dumb_projectile_ok(&self) -> bool {
        self.honesty_howitzer_shell_ok()
            && self.orbit_fields.iter().any(|f| {
                f.howitzer_shell_dumb_projectile_applications > 0
                    && f.howitzer_shell_physics_mass_applications > 0
                    && f.howitzer_shell_death_detonated_applications > 0
                    && f.howitzer_shell_death_lasered_applications > 0
                    && f.howitzer_shell_death_lasered_ocl_applications > 0
                    && f.howitzer_shell_death_generic_applications > 0
                    && f.howitzer_shell_only_moving_down_applications > 0
                    && f.howitzer_shell_model_draw_applications > 0
                    && f.howitzer_shell_scale_applications > 0
            })
            && (SPECTRE_HOWITZER_SHELL_GEOMETRY_HEIGHT - 4.0).abs() < 0.01
            && (SPECTRE_HOWITZER_SHELL_LOCOMOTOR_SPEED - 1111.0).abs() < 0.01
            && SPECTRE_HOWITZER_SHELL_DEATH_GENERIC_FX.contains("GenericMissileDeath")
    }

    /// Residual honesty: SpectreHowitzerShell InstantDeath GENERIC residual.
    ///
    /// Tracks FX_GenericMissileDeath residual path (ALL -LASERED -DETONATED).
    /// Fail-closed: not full InstantDeathBehavior Object / live OCL spawn matrix.
    pub fn honesty_howitzer_shell_death_generic_ok(&self) -> bool {
        self.orbit_fields.iter().any(|f| {
            f.howitzer_shell_death_generic_applications > 0
                && f.howitzer_shell_death_generic_applications >= f.howitzer_shells_spawned
        }) && SPECTRE_HOWITZER_SHELL_DEATH_GENERIC_FX == "FX_GenericMissileDeath"
    }

    /// Residual honesty: SpectreHowitzerShell design-params residual.
    ///
    /// Tracks TargetHeightIncludesStructures **No**, InitialHealth **100**,
    /// DisplayName **OBJECT:Missile**, EditorSorting **SYSTEM**, OkToChangeModelColor.
    /// Fail-closed: not full ThingFactory Object / HeightDie module matrix.
    pub fn honesty_howitzer_shell_design_params_ok(&self) -> bool {
        self.orbit_fields.iter().any(|f| {
            f.howitzer_shell_design_params_applications > 0
                && f.howitzer_shell_design_params_applications >= f.howitzer_shells_spawned
        }) && !SPECTRE_HOWITZER_SHELL_HEIGHT_DIE_INCLUDES_STRUCTURES
            && (SPECTRE_HOWITZER_SHELL_INITIAL_HEALTH - 100.0).abs() < 0.01
            && (SPECTRE_HOWITZER_SHELL_INITIAL_HEALTH - SPECTRE_HOWITZER_SHELL_MAX_HEALTH).abs()
                < 0.01
            && SPECTRE_HOWITZER_SHELL_DISPLAY_NAME == "OBJECT:Missile"
            && SPECTRE_HOWITZER_SHELL_EDITOR_SORTING == "SYSTEM"
            && SPECTRE_HOWITZER_SHELL_OK_TO_CHANGE_MODEL_COLOR
    }

    /// Residual honesty: SpectreHowitzerShell W3D ModelDraw residual.
    ///
    /// Tracks model AVSpectreShell1 + Scale/Shadow/Geometry/MaxHealth honesty
    /// per shell spawn. Fail-closed: not full W3D drawable Object / GPU mesh submit.
    pub fn honesty_howitzer_shell_model_draw_ok(&self) -> bool {
        self.orbit_fields.iter().any(|f| {
            f.howitzer_shell_model_draw_applications > 0
                && f.howitzer_shell_scale_applications >= f.howitzer_shell_model_draw_applications
                && f.howitzer_shell_shadow_applications >= f.howitzer_shell_model_draw_applications
                && f.howitzer_shell_geometry_applications
                    >= f.howitzer_shell_model_draw_applications
                && f.howitzer_shell_max_health_applications
                    >= f.howitzer_shell_model_draw_applications
        }) && SPECTRE_HOWITZER_SHELL_MODEL == "AVSpectreShell1"
            && (SPECTRE_HOWITZER_SHELL_SCALE - 0.6).abs() < 0.01
            && SPECTRE_HOWITZER_SHELL_SHADOW == "SHADOW_DECAL"
            && SPECTRE_HOWITZER_SHELL_GEOMETRY == "Cylinder"
            && SPECTRE_HOWITZER_SHELL_GEOMETRY_IS_SMALL
            && (SPECTRE_HOWITZER_SHELL_MAX_HEALTH - 100.0).abs() < 0.01
            && (SPECTRE_HOWITZER_SHELL_GEOMETRY_RADIUS - 4.0).abs() < 0.01
    }

    /// Residual honesty: MODELCONDITION_CONTINUOUS_FIRE_MEAN/FAST residual sets.
    ///
    /// Fail-closed: not full drawable animation state / W3D model condition matrix.
    /// Residual honesty: SpectreHowitzerShell KindOf / VisionRange / Armor residual.
    ///
    /// Tracks KindOf PROJECTILE, VisionRange **0**, Armor ProjectileArmor.
    /// Fail-closed: not full ThingFactory Object / ArmorSet module matrix.
    pub fn honesty_howitzer_shell_object_params_ok(&self) -> bool {
        self.orbit_fields.iter().any(|f| {
            f.howitzer_shell_object_params_applications > 0
                && f.howitzer_shell_object_params_applications >= f.howitzer_shells_spawned
        }) && SPECTRE_HOWITZER_SHELL_KIND_OF == "PROJECTILE"
            && (SPECTRE_HOWITZER_SHELL_VISION_RANGE - 0.0).abs() < 0.01
            && SPECTRE_HOWITZER_SHELL_ARMOR == "ProjectileArmor"
    }

    pub fn honesty_model_condition_continuous_fire_ok(&self) -> bool {
        self.orbit_fields
            .iter()
            .any(|f| f.model_condition_mean_sets > 0 || f.model_condition_fast_sets > 0)
    }

    /// Residual honesty: MODELCONDITION_CONTINUOUS_FIRE_SLOW residual on coolDown.
    pub fn honesty_model_condition_slow_ok(&self) -> bool {
        self.orbit_fields
            .iter()
            .any(|f| f.model_condition_slow_sets > 0)
    }

    /// True if at least one strike of `kind` is currently queued.
    pub fn honesty_queue_ok(&self, kind: HostSuperweaponKind) -> bool {
        !self.pending_of_kind(kind).is_empty()
    }

    /// True if at least one strike of `kind` completed with damage applied
    /// (or completed cleanly with zero victims in radius — still "completed").
    pub fn honesty_complete_ok(&self, kind: HostSuperweaponKind) -> bool {
        self.completed_of_kind(kind)
            .iter()
            .any(|s| s.phase == HostStrikePhase::Completed)
    }

    /// True if at least one residual radiation field was spawned this session.
    pub fn honesty_radiation_ok(&self) -> bool {
        self.radiation_fields_spawned_total > 0
            || !self.radiation_fields.is_empty()
            || !self.radiation_spawned_this_frame.is_empty()
    }

    /// Stronger radiation honesty: residual field applied at least one damage tick.
    pub fn honesty_radiation_damage_ok(&self) -> bool {
        self.radiation_damage_applications_total > 0
            || self
                .radiation_fields
                .iter()
                .any(|f| f.damage_applications > 0 || f.total_damage_applied > 0.0)
    }

    /// True if at least one residual toxin field was spawned this session.
    pub fn honesty_toxin_ok(&self) -> bool {
        self.toxin_fields_spawned_total > 0
            || !self.toxin_fields.is_empty()
            || !self.toxin_spawned_this_frame.is_empty()
    }

    /// Stronger toxin honesty: residual field applied at least one damage tick.
    pub fn honesty_toxin_damage_ok(&self) -> bool {
        self.toxin_damage_applications_total > 0
            || self
                .toxin_fields
                .iter()
                .any(|f| f.damage_applications > 0 || f.total_damage_applied > 0.0)
    }

    /// True if at least one residual Spectre orbit field was spawned this session.
    pub fn honesty_orbit_ok(&self) -> bool {
        self.orbit_fields_spawned_total > 0
            || !self.orbit_fields.is_empty()
            || !self.orbit_spawned_this_frame.is_empty()
    }

    /// Stronger orbit honesty: residual field applied at least one damage tick.
    pub fn honesty_orbit_damage_ok(&self) -> bool {
        self.orbit_damage_applications_total > 0
            || self
                .orbit_fields
                .iter()
                .any(|f| f.damage_applications > 0 || f.total_damage_applied > 0.0)
    }

    /// True if at least one residual Particle Uplink beam field was spawned.
    pub fn honesty_beam_ok(&self) -> bool {
        self.beam_fields_spawned_total > 0
            || !self.beam_fields.is_empty()
            || !self.beam_spawned_this_frame.is_empty()
    }

    /// Stronger beam honesty: residual field applied at least one damage pulse.
    pub fn honesty_beam_damage_ok(&self) -> bool {
        self.beam_damage_applications_total > 0
            || self
                .beam_fields
                .iter()
                .any(|f| f.damage_applications > 0 || f.total_damage_applied > 0.0)
    }

    /// Residual honesty: SwathOfDeath epicenter walked off the click point.
    pub fn honesty_beam_swath_ok(&self) -> bool {
        self.beam_fields.iter().any(|f| f.swath_applications > 0)
            || self.beam_fields.iter().any(|f| f.max_swath_offset > 0.01)
    }

    /// Residual honesty: DamagePulseRemnant trail residual spawned from beam pulses.
    pub fn honesty_beam_remnant_ok(&self) -> bool {
        self.remnant_fields_spawned_total > 0 || !self.remnant_fields.is_empty()
    }

    /// Residual honesty: remnant trail applied damage at least once.
    pub fn honesty_beam_remnant_damage_ok(&self) -> bool {
        self.remnant_damage_applications_total > 0
            || self
                .remnant_fields
                .iter()
                .any(|f| f.damage_applications > 0)
    }

    /// Residual honesty: WidthGrow damage-radius residual ramped past a floor.
    ///
    /// True when any beam field reached width scalar ≥ 0.5 (half WidthGrowTime).
    /// Fail-closed: not full GPU laser width matrix / OuterBeamWidth × scalar.
    pub fn honesty_beam_width_grow_ok(&self) -> bool {
        self.beam_fields.iter().any(|f| f.peak_width_scalar >= 0.5)
            && PARTICLE_WIDTH_GROW_FRAMES == 60
    }

    /// Residual honesty: WidthGrow decay shrink residual after TotalFiringTime.
    ///
    /// True when any beam field sampled decay (trough scalar ≤ 0.5 after a
    /// full peak). Fail-closed: not full OuterBeamWidth GPU laser / drawable
    /// destroy after orbitalDeathFrame client path.
    pub fn honesty_beam_width_decay_ok(&self) -> bool {
        self.beam_fields.iter().any(|f| {
            f.decay_samples > 0
                && f.trough_width_scalar <= 0.5 + f32::EPSILON
                && f.peak_width_scalar >= 0.99
        }) && PARTICLE_WIDTH_GROW_FRAMES == 60
            && PARTICLE_BEAM_ORBITAL_LIFETIME_FRAMES
                == PARTICLE_BEAM_DURATION_FRAMES + PARTICLE_WIDTH_GROW_FRAMES
    }

    /// Residual honesty: multi-beam NumBeams + ScrollRate / TilingScalar residual.
    ///
    /// Tracks W3DLaserDraw NumBeams **12**, ScrollRate UV accumulation, and
    /// TilingScalar honesty on a live beam field. Fail-closed: not full GPU
    /// multi-beam soft edge / texture atlas submit (soft-edge residual closed
    /// separately via [`honesty_beam_soft_edge_ok`]).
    pub fn honesty_beam_num_beams_scroll_ok(&self) -> bool {
        self.beam_fields.iter().any(|f| {
            f.num_beams_armed == PARTICLE_ORBITAL_LASER_NUM_BEAMS
                && f.tiling_scalar_armed >= 1
                && f.scroll_uv_samples >= 1
                && f.peak_abs_scroll_uv > 0.0
        }) && PARTICLE_ORBITAL_LASER_NUM_BEAMS == 12
            && (PARTICLE_ORBITAL_LASER_SCROLL_RATE + 1.75).abs() < 0.01
            && (PARTICLE_ORBITAL_LASER_TILING_SCALAR - 0.15).abs() < 0.01
            && particle_orbital_laser_num_beams() == 12
            && (particle_orbital_laser_tiling_scalar() - 0.15).abs() < 0.01
            // 30 frames at ScrollRate -1.75 → UV = -1.75
            && (particle_orbital_laser_scroll_uv(0, 30) + 1.75).abs() < 0.01
    }

    /// Residual honesty: multi-beam soft-edge width/alpha/color/tile residual.
    ///
    /// Tracks W3DLaserDraw cylinder soft edge (`scale = i/(NumBeams-1)`),
    /// InnerColor/OuterColor lerp, and tile-factor honesty. Fail-closed: not
    /// full SegLineRenderer GPU texture atlas submit / live surface aspect.
    pub fn honesty_beam_soft_edge_ok(&self) -> bool {
        self.beam_fields.iter().any(|f| {
            f.soft_edge_color_armed >= 1
                && f.soft_edge_samples >= 1
                && f.peak_soft_edge_outer_width > 0.0
                && f.last_soft_edge_outer_alpha > 0.0
                && f.last_soft_edge_tile_factor > 0.0
        }) && PARTICLE_ORBITAL_LASER_NUM_BEAMS == 12
            && (particle_orbital_soft_edge_scale(0) - 0.0).abs() < 0.01
            && (particle_orbital_soft_edge_scale(11) - 1.0).abs() < 0.01
            && (particle_orbital_soft_edge_outer_width_peak() - 26.0).abs() < 0.01
            && (particle_orbital_soft_edge_alpha(0) - PARTICLE_ORBITAL_LASER_INNER_COLOR.3).abs()
                < 0.01
            && (particle_orbital_soft_edge_alpha(11) - PARTICLE_ORBITAL_LASER_OUTER_COLOR.3).abs()
                < 0.01
            && PARTICLE_ORBITAL_LASER_TILE
            && PARTICLE_ORBITAL_LASER_TEXTURE.contains("EXNoise")
            && (PARTICLE_ORBITAL_LASER_INNER_COLOR.0 - 1.0).abs() < 0.01
            && (PARTICLE_ORBITAL_LASER_OUTER_COLOR.2 - 1.0).abs() < 0.01
    }

    /// Residual honesty: OuterBeamWidth × width_scalar orbital laser residual.
    ///
    /// Tracks W3DLaserDraw OuterBeamWidth draw width, `getCurrentLaserRadius`
    /// (OuterBeamWidth×0.5×scalar), and retail damage formula
    /// (laser radius × DamageRadiusScalar = peak 44.2). Host combat damage
    /// uses [`PARTICLE_BEAM_RADIUS`] (44.2). Fail-closed: not full GPU
    /// multi-beam soft edge / texture atlas submit (NumBeams residual closed
    /// separately via [`honesty_beam_num_beams_scroll_ok`]).
    /// Residual honesty: soft-edge RGB innerAlpha premultiply residual.
    ///
    /// Tracks C++ W3DLaserDraw channel-delta × innerAlpha on outer cylinder.
    /// Fail-closed: not full SegLineRenderer additive GPU submit.
    pub fn honesty_beam_soft_edge_premul_ok(&self) -> bool {
        self.beam_fields
            .iter()
            .any(|f| f.soft_edge_premul_samples >= 1 && f.soft_edge_color_armed >= 1)
            && {
                let (r0, _, _, _) = particle_orbital_soft_edge_color_premul(0);
                let (r11, _, _, a11) = particle_orbital_soft_edge_color_premul(11);
                // Outer red at scale=1: 1.0 + 1.0*(0-1)*ia = 1 - ia
                let ia = PARTICLE_ORBITAL_LASER_INNER_COLOR.3;
                (r0 - 1.0).abs() < 0.01
                    && (r11 - (1.0 - ia)).abs() < 0.01
                    && (a11 - PARTICLE_ORBITAL_LASER_OUTER_COLOR.3).abs() < 0.01
            }
    }

    /// Residual honesty: single-beam RGB × innerAlpha residual (NumBeams==1 path).
    ///
    /// Fail-closed: not full SegLineRenderer GPU submit (OrbitalLaser multi-beam).
    pub fn honesty_beam_single_beam_premul_ok(&self) -> bool {
        let (r, g, b, a) = particle_orbital_single_beam_color_premul();
        let ia = PARTICLE_ORBITAL_LASER_INNER_COLOR.3;
        (r - ia).abs() < 0.01
            && (g - ia).abs() < 0.01
            && (b - ia).abs() < 0.01
            && (a - ia).abs() < 0.01
    }

    /// Residual honesty: intense connector soft-edge RGB innerAlpha premul residual.
    ///
    /// Tracks C++ W3DLaserDraw channel-delta × innerAlpha on connector cylinders.
    /// Fail-closed: not full LaserUpdate drawable / GPU SegLine submit.
    pub fn honesty_beam_connector_soft_edge_premul_ok(&self) -> bool {
        self.beam_fields
            .iter()
            .any(|f| f.connector_soft_edge_premul_samples >= 1 && f.connector_soft_edge_armed >= 1)
            && {
                let ia = PARTICLE_CONNECTOR_INNER_COLOR.3;
                let (r0, _, _, _) = particle_connector_intense_soft_edge_color_premul(0);
                let (r4, _, _, a4) = particle_connector_intense_soft_edge_color_premul(4);
                // Outer red at scale=1: 1 + (0-1)*ia = 1 - ia (connector outer is pure blue).
                (r0 - 1.0).abs() < 0.01
                    && (r4 - (1.0 - ia)).abs() < 0.01
                    && (a4 - PARTICLE_CONNECTOR_OUTER_COLOR.3).abs() < 0.01
            }
    }

    /// Residual honesty: OrbitalLaser KindOf IMMOBILE + Segments/ArcHeight residual.
    ///
    /// Tracks KindOf **IMMOBILE**, Segments **1**, ArcHeight **0** design defaults.
    /// Fail-closed: not full ThingFactory Object / multi-segment arc LaserUpdate.
    pub fn honesty_beam_orbital_kindof_segments_ok(&self) -> bool {
        self.beam_fields.iter().any(|f| {
            f.orbital_kindof_immobile_armed >= 1
                && f.orbital_segments_armed == PARTICLE_ORBITAL_LASER_SEGMENTS
                && f.orbital_arc_height_armed >= 1
        }) && PARTICLE_ORBITAL_LASER_KIND_OF == "IMMOBILE"
            && PARTICLE_ORBITAL_LASER_SEGMENTS == 1
            && (PARTICLE_ORBITAL_LASER_ARC_HEIGHT - 0.0).abs() < 0.01
            && (PARTICLE_ORBITAL_LASER_SEGMENT_OVERLAP - 0.0).abs() < 0.01
    }

    pub fn honesty_beam_outer_beam_width_ok(&self) -> bool {
        self.beam_fields.iter().any(|f| {
            f.orbital_laser_draw_params_armed >= 1
                && f.connector_outer_beam_width_armed >= 1
                && f.peak_outer_beam_draw_width
                    >= PARTICLE_ORBITAL_LASER_OUTER_BEAM_WIDTH * 0.5 - f32::EPSILON
                && f.peak_retail_laser_radius
                    >= particle_orbital_laser_template_width() * 0.5 - f32::EPSILON
                && f.peak_retail_damage_radius
                    >= particle_orbital_laser_template_width() * 0.5 * PARTICLE_DAMAGE_RADIUS_SCALAR
                        - 0.1
        }) && (PARTICLE_ORBITAL_LASER_OUTER_BEAM_WIDTH - 26.0).abs() < 0.01
            && (PARTICLE_ORBITAL_LASER_INNER_BEAM_WIDTH - 0.6).abs() < 0.01
            && PARTICLE_ORBITAL_LASER_NUM_BEAMS == 12
            && (PARTICLE_CONNECTOR_INTENSE_OUTER_BEAM_WIDTH - 2.0).abs() < 0.01
            && (PARTICLE_CONNECTOR_MEDIUM_OUTER_BEAM_WIDTH - 1.2).abs() < 0.01
            && PARTICLE_ORBITAL_LASER_TEXTURE.contains("EXNoise")
            && (particle_orbital_laser_template_width() - 13.0).abs() < 0.01
            && (particle_retail_damage_radius(0, PARTICLE_WIDTH_GROW_FRAMES) - 44.2).abs() < 0.05
    }

    /// Residual honesty: manual beam drive moved the epicenter at least once.
    ///
    /// Fail-closed: not full scripted waypoint mode / disabled-object reject /
    /// terrain height snap on every frame.
    pub fn honesty_beam_manual_drive_ok(&self) -> bool {
        self.beam_fields
            .iter()
            .any(|f| f.manual_target_mode && f.manual_drive_distance_total > 0.01)
            && (PARTICLE_MANUAL_DRIVING_SPEED - 20.0).abs() < 0.01
    }

    /// Residual honesty: ManualFastDrivingSpeed used after double-click residual.
    pub fn honesty_beam_fast_drive_ok(&self) -> bool {
        self.beam_fields
            .iter()
            .any(|f| f.fast_drive_applications > 0)
            && (PARTICLE_MANUAL_FAST_DRIVING_SPEED - 40.0).abs() < 0.01
            && PARTICLE_DOUBLE_CLICK_FAST_DRIVE_FRAMES == 15
    }

    /// Residual honesty: STATUS_FIRING outer-node + connector laser residual.
    ///
    /// Fail-closed: not full W3D bone-world convert / live LaserUpdate drawable
    /// matrix (bone layout residual closed via
    /// [`honesty_beam_outer_node_bone_layout_ok`]; intensity schedule residual
    /// closed separately via [`honesty_beam_intensity_schedule_ok`]).
    pub fn honesty_beam_outer_nodes_ok(&self) -> bool {
        self.beam_fields.iter().any(|f| {
            f.outer_node_systems_created == PARTICLE_OUTER_EFFECT_NUM_BONES
                && f.connector_lasers_created == PARTICLE_OUTER_EFFECT_NUM_BONES
                && f.laser_base_flare_created >= 1
                && f.ground_to_orbit_laser_created >= 1
        }) && PARTICLE_OUTER_EFFECT_NUM_BONES == 5
            && PARTICLE_OUTER_NODE_INTENSE_FLARE.contains("Intense")
            && PARTICLE_CONNECTOR_INTENSE_LASER.contains("Intense")
            && PARTICLE_ORBITAL_LASER_NAME.contains("OrbitalLaser")
    }

    /// Residual honesty: outer-node FX01..FX05 bone layout + connector residual.
    ///
    /// Host residual places bones on a ring around the building origin
    /// (fail-closed vs full W3D bone-world matrix extract / dish mesh attach).
    pub fn honesty_beam_outer_node_bone_layout_ok(&self) -> bool {
        self.beam_fields.iter().any(|f| {
            f.outer_node_bone_layout_applications == PARTICLE_OUTER_EFFECT_NUM_BONES
                && f.connector_bone_layout_applications >= 1
                && f.last_outer_node_bone_position != Vec3::ZERO
        }) && PARTICLE_OUTER_EFFECT_NUM_BONES == 5
            && particle_outer_node_bone_name(0) == "FX01"
            && particle_outer_node_bone_name(4) == "FX05"
            && PARTICLE_CONNECTOR_BONE_NAME == "FXConnector"
            && PARTICLE_FIRE_BONE_NAME == "FXMain"
            && (PARTICLE_OUTER_NODE_RING_RADIUS - 40.0).abs() < 0.01
            && (PARTICLE_OUTER_NODE_RING_HEIGHT - 25.0).abs() < 0.01
            && PARTICLE_CONNECTOR_INTENSE_NUM_BEAMS == 5
            && PARTICLE_CONNECTOR_MEDIUM_NUM_BEAMS == 4
            && PARTICLE_CONNECTOR_LASER_TEXTURE.contains("EXLaser")
    }

    /// Residual honesty: intense connector soft-edge + laser segments residual.
    ///
    /// Tracks NumBeams **5** width/color lerp and outer-node→connector segments.
    /// Fail-closed: not full LaserUpdate drawable matrix / client shroud path.
    pub fn honesty_beam_connector_soft_edge_ok(&self) -> bool {
        self.beam_fields.iter().any(|f| {
            f.connector_soft_edge_armed >= 1
                && f.connector_laser_segments_created == PARTICLE_OUTER_EFFECT_NUM_BONES
                && (f.peak_connector_soft_edge_outer_width
                    - PARTICLE_CONNECTOR_INTENSE_OUTER_BEAM_WIDTH)
                    .abs()
                    < 0.01
                && f.last_connector_segment_end != Vec3::ZERO
        }) && PARTICLE_CONNECTOR_INTENSE_NUM_BEAMS == 5
            && (particle_connector_intense_soft_edge_scale(0) - 0.0).abs() < 0.01
            && (particle_connector_intense_soft_edge_scale(4) - 1.0).abs() < 0.01
            && (particle_connector_intense_soft_edge_width(4) - 2.0).abs() < 0.01
            && (particle_connector_intense_soft_edge_width(0) - 0.6).abs() < 0.01
            && PARTICLE_CONNECTOR_LASER_TEXTURE == "EXLaser.tga"
            && (PARTICLE_CONNECTOR_INTENSE_INNER_BEAM_WIDTH - 0.6).abs() < 0.01
            && (PARTICLE_CONNECTOR_MEDIUM_INNER_BEAM_WIDTH - 0.4).abs() < 0.01
    }

    /// Residual honesty: CHARGING/PREPARING/ALMOST_READY/READY intensity schedule.
    ///
    /// True when a ParticleCannon strike observed at least PREPARING residual
    /// (or ALMOST_READY when impact_delay only covers the late window) and
    /// BeamLaunchFX / POSTFIRE intensity residual exists on a beam field.
    /// Fail-closed: not full W3D bone extract / live ParticleSystem manager.
    /// Residual honesty: Medium connector soft-edge residual (POSTFIRE Medium).
    ///
    /// Tracks NumBeams **4**, Inner **0.4** → Outer **1.2**, soft-edge scale/color.
    /// Fail-closed: not full LaserUpdate drawable matrix / client shroud path.
    pub fn honesty_beam_connector_medium_soft_edge_ok(&self) -> bool {
        self.beam_fields.iter().any(|f| {
            f.medium_connector_soft_edge_armed >= 1
                && (f.peak_medium_connector_soft_edge_outer_width
                    - PARTICLE_CONNECTOR_MEDIUM_OUTER_BEAM_WIDTH)
                    .abs()
                    < 0.01
        }) && PARTICLE_CONNECTOR_MEDIUM_NUM_BEAMS == 4
            && (PARTICLE_CONNECTOR_MEDIUM_INNER_BEAM_WIDTH - 0.4).abs() < 0.01
            && (PARTICLE_CONNECTOR_MEDIUM_OUTER_BEAM_WIDTH - 1.2).abs() < 0.01
            && (particle_connector_medium_soft_edge_scale(0) - 0.0).abs() < 0.01
            && (particle_connector_medium_soft_edge_scale(3) - 1.0).abs() < 0.01
            && (particle_connector_medium_soft_edge_width(0) - 0.4).abs() < 0.01
            && (particle_connector_medium_soft_edge_width(3) - 1.2).abs() < 0.01
            && PARTICLE_CONNECTOR_LASER_TEXTURE == "EXLaser.tga"
    }

    /// Residual honesty: OrbitalLaser VisionRange / ShroudClearing residual.
    ///
    /// Tracks retail design VisionRange **100** / ShroudClearingRange **120**
    /// armed at STATUS_FIRING. Fail-closed: not full client FOW reveal grid path.
    pub fn honesty_beam_vision_shroud_ok(&self) -> bool {
        self.beam_fields.iter().any(|f| {
            f.orbital_vision_shroud_armed >= 1
                && (f.last_orbital_vision_range - PARTICLE_ORBITAL_LASER_VISION_RANGE).abs() < 0.01
                && (f.last_orbital_shroud_clearing_range
                    - PARTICLE_ORBITAL_LASER_SHROUD_CLEARING_RANGE)
                    .abs()
                    < 0.01
        }) && (PARTICLE_ORBITAL_LASER_VISION_RANGE - 100.0).abs() < 0.01
            && (PARTICLE_ORBITAL_LASER_SHROUD_CLEARING_RANGE - 120.0).abs() < 0.01
    }

    pub fn honesty_beam_intensity_schedule_ok(&self) -> bool {
        // Pre-fire residual: host impact_delay (BeamTravelTime 75f) only covers
        // PREPARING→ALMOST_READY→READY (full CHARGING needs BeginCharge+RaiseAntenna
        // windows that exceed impact_delay).
        let strike_ok = self.strikes.values().any(|s| {
            s.kind == HostSuperweaponKind::ParticleCannon
                && s.particle_intensity_transitions >= 1
                && (s.particle_preparing_applications > 0
                    || s.particle_almost_ready_applications > 0
                    || s.particle_ready_applications > 0
                    || s.particle_charging_applications > 0)
                && s.particle_status_peak.as_u8() >= ParticleUplinkStatus::Preparing.as_u8()
        });
        let beam_ok = self.beam_fields.iter().any(|f| {
            f.beam_launch_fx_applications >= 1
                && f.outer_intensity == ParticleIntensity::Intense
                && PARTICLE_LAUNCH_FX_INTERVAL_FRAMES == 30
                && PARTICLE_BEAM_LAUNCH_FX.contains("BeamLaunch")
        });
        let timing_ok = PARTICLE_BEGIN_CHARGE_FRAMES == 150
            && PARTICLE_RAISE_ANTENNA_FRAMES == 140
            && PARTICLE_READY_DELAY_FRAMES == 60
            && PARTICLE_BEAM_TRAVEL_FRAMES == 75;
        (strike_ok || beam_ok) && timing_ok
    }

    /// Residual honesty: POSTFIRE Medium intensity after TotalFiringTime.
    pub fn honesty_beam_postfire_ok(&self) -> bool {
        self.beam_fields.iter().any(|f| {
            f.postfire_applications > 0
                && (f.status == ParticleUplinkStatus::Postfire
                    || f.status == ParticleUplinkStatus::Packing
                    || f.outer_intensity == ParticleIntensity::Medium)
        })
    }

    /// Residual honesty: BeamLaunchFX refresh residual while STATUS_FIRING.
    pub fn honesty_beam_launch_fx_ok(&self) -> bool {
        self.beam_fields
            .iter()
            .any(|f| f.beam_launch_fx_applications >= 2)
            && PARTICLE_LAUNCH_FX_INTERVAL_FRAMES == 30
    }

    /// Residual honesty: ScudStorm PreAttack + Chem FXBone residual.
    ///
    /// Fail-closed: not full ScudStormMissile ThingFactory Object path.
    pub fn honesty_scud_pre_attack_and_chem_fx_ok(&self) -> bool {
        self.strikes.values().any(|s| {
            s.kind == HostSuperweaponKind::ScudStorm
                && s.scud_chem_fx_bones == SCUD_STORM_CHEM_FX_BONE_COUNT
                && s.scud_launch_bone_applications >= 1
                && (s.scud_pre_attack_frames > 0
                    || s.scud_fire_fx_applications > 0
                    || s.scud_pre_attack_active)
        }) && SCUD_STORM_CHEM_FX_BONE_COUNT == 3
            && SCUD_STORM_CHEM_FX_PARTICLE.contains("Goo")
            && SCUD_STORM_FIRE_FX.contains("ScudStormMissile")
            && SCUD_STORM_LAUNCH_BONE == "WeaponA"
    }

    /// Residual honesty: ScudStormMissile MissileAIUpdate loft residual.
    ///
    /// Tracks loft / IgnitionFX / FireSound / exhaust / HeightDie /
    /// SpecialPowerCompletionDie residual per missile wave. Fail-closed: not
    /// full ThingFactory projectile Object / live MissileAIUpdate physics sim
    /// (PreferredHeight spring residual closed separately via
    /// [`honesty_scud_preferred_height_spring_ok`]).
    pub fn honesty_scud_missile_loft_ok(&self) -> bool {
        self.strikes.values().any(|s| {
            s.kind == HostSuperweaponKind::ScudStorm
                && s.scud_missile_loft_applications > 0
                && s.scud_ignition_fx_applications >= s.scud_missile_loft_applications
                && s.scud_launch_sound_applications >= s.scud_missile_loft_applications
                && s.scud_exhaust_applications >= s.scud_missile_loft_applications
                && s.scud_height_die_applications >= s.scud_missile_loft_applications
                && s.scud_special_power_completion_applications >= s.scud_missile_loft_applications
                && s.scud_fire_fx_applications >= s.scud_missile_loft_applications
        }) && SCUD_STORM_MISSILE_OBJECT == "ScudStormMissile"
            && !SCUD_STORM_MISSILE_TRY_FOLLOW_TARGET
            && SCUD_STORM_MISSILE_FUEL_LIFETIME == 0
            && (SCUD_STORM_MISSILE_DISTANCE_BEFORE_TURNING - 500.0).abs() < 0.01
            && (SCUD_STORM_MISSILE_DISTANCE_BEFORE_DIVING - 200.0).abs() < 0.01
            && (SCUD_STORM_MISSILE_HEIGHT_DIE_TARGET - 15.0).abs() < 0.01
            && SCUD_STORM_MISSILE_HEIGHT_DIE_INITIAL_DELAY_FRAMES == 30
            && (SCUD_STORM_MISSILE_PREFERRED_HEIGHT - 240.0).abs() < 0.01
            && (SCUD_STORM_MISSILE_LOCOMOTOR_SPEED - 300.0).abs() < 0.01
            && (SCUD_STORM_MISSILE_MASS - 500.0).abs() < 0.01
            && SCUD_STORM_MISSILE_IGNITION_FX.contains("Ignition")
            && SCUD_STORM_MISSILE_LAUNCH_SOUND.contains("Launch")
            && SCUD_STORM_MISSILE_EXHAUST.contains("Exhaust")
            && SCUD_STORM_MISSILE_SPECIAL_POWER.contains("ScudStorm")
    }

    /// Residual honesty: ScudStormMissile PreferredHeight spring residual.
    ///
    /// Tracks spawn-at-PreferredHeight, Locomotor damping spring samples, and
    /// loft phase peak (Loft→Turn→Dive→HeightDie). Fail-closed: not full
    /// ThingFactory Object / live physics flight path (ballistic residual closed
    /// separately via [`honesty_scud_ballistic_flight_ok`]).
    pub fn honesty_scud_preferred_height_spring_ok(&self) -> bool {
        self.strikes.values().any(|s| {
            s.kind == HostSuperweaponKind::ScudStorm
                && s.scud_spawn_height_applications > 0
                && s.scud_preferred_height_spring_applications
                    >= s.scud_spawn_height_applications
                && s.scud_loft_phase_peak.as_u8() >= ScudMissileLoftPhase::HeightDie.as_u8()
                && s.scud_last_spring_height > 0.0
        }) && (scud_missile_spawn_height() - 240.0).abs() < 0.01
            && (SCUD_STORM_MISSILE_PREFERRED_HEIGHT_DAMPING - 0.7).abs() < 0.01
            // One spring step from ground: 0 + (240-0)*0.7 = 168.
            && (scud_missile_preferred_height_spring(0.0) - 168.0).abs() < 0.01
            // Already at preferred: spring holds height.
            && (scud_missile_preferred_height_spring(240.0) - 240.0).abs() < 0.01
            && scud_missile_loft_phase(0.0, 1000.0, 100.0) == ScudMissileLoftPhase::Loft
            && scud_missile_loft_phase(500.0, 1000.0, 200.0) == ScudMissileLoftPhase::Turn
            && scud_missile_loft_phase(600.0, 100.0, 100.0) == ScudMissileLoftPhase::Dive
            && scud_missile_loft_phase(600.0, 50.0, 10.0) == ScudMissileLoftPhase::HeightDie
    }

    /// Residual honesty: ScudStormMissile ballistic flight residual.
    ///
    /// Tracks locomotor speed/accel path sampling, OnlyWhenMovingDown,
    /// SnapToGroundOnDeath, and W3D model residual. Fail-closed: not full
    /// ThingFactory Object / live Physics motive force / turn-rate matrix.
    pub fn honesty_scud_ballistic_flight_ok(&self) -> bool {
        self.strikes.values().any(|s| {
            s.kind == HostSuperweaponKind::ScudStorm
                && s.scud_ballistic_flight_applications > 0
                && s.scud_only_moving_down_applications >= s.scud_ballistic_flight_applications
                && s.scud_snap_to_ground_applications >= s.scud_ballistic_flight_applications
                && s.scud_model_draw_applications >= s.scud_ballistic_flight_applications
                && s.scud_peak_flight_distance > 0.0
                && s.scud_loft_phase_peak.as_u8() >= ScudMissileLoftPhase::HeightDie.as_u8()
        }) && SCUD_STORM_MISSILE_MODEL == "UBScudStrm_M"
            && SCUD_STORM_MISSILE_HEIGHT_DIE_ONLY_MOVING_DOWN
            && SCUD_STORM_MISSILE_SNAP_TO_GROUND_ON_DEATH
            && SCUD_STORM_MISSILE_HEIGHT_DIE_INCLUDES_STRUCTURES
            && (SCUD_STORM_MISSILE_LOCOMOTOR_SPEED - 300.0).abs() < 0.01
            && (SCUD_STORM_MISSILE_LOCOMOTOR_ACCEL - 675.0).abs() < 0.01
            && (SCUD_STORM_MISSILE_LOCOMOTOR_SPEED_DAMAGED - 200.0).abs() < 0.01
            && (SCUD_STORM_MISSILE_LOCOMOTOR_MIN_SPEED - 100.0).abs() < 0.01
            && (SCUD_STORM_MISSILE_LOCOMOTOR_TURN_RATE - 540.0).abs() < 0.01
            && (SCUD_STORM_MISSILE_LOCOMOTOR_MAX_THRUST_ANGLE - 45.0).abs() < 0.01
            && (SCUD_STORM_MISSILE_MAX_HEALTH - 10000.0).abs() < 0.01
            && SCUD_STORM_MISSILE_GEOMETRY_IS_SMALL
            && (scud_missile_speed_per_frame() - 10.0).abs() < 0.01
    }

    /// Residual honesty: ScudStormMissile ThrustRoll / ThrustWobble residual.
    ///
    /// Tracks locomotor thrust wobble samples on ballistic flight residual.
    /// Fail-closed: not full Locomotor thrust matrix / Physics motive force.
    pub fn honesty_scud_thrust_wobble_ok(&self) -> bool {
        self.strikes.values().any(|s| {
            s.kind == HostSuperweaponKind::ScudStorm
                && s.scud_thrust_wobble_applications > 0
                && s.scud_peak_abs_thrust_wobble > 0.0
        }) && (SCUD_STORM_MISSILE_THRUST_ROLL - 0.06).abs() < 0.001
            && (SCUD_STORM_MISSILE_THRUST_WOBBLE_RATE - 0.008).abs() < 0.001
            && (SCUD_STORM_MISSILE_THRUST_MIN_WOBBLE + 0.040).abs() < 0.001
            && (SCUD_STORM_MISSILE_THRUST_MAX_WOBBLE - 0.040).abs() < 0.001
            && SCUD_STORM_MISSILE_CLOSE_ENOUGH_DIST_3D
            && scud_missile_thrust_wobble(0).abs() <= 0.040 + f32::EPSILON
    }

    /// Residual honesty: ScudStormMissile Geometry residual.
    ///
    /// Tracks Cylinder / GeometryIsSmall / MajorRadius **7** / Height **30** /
    /// Mass **500** / MaxHealth **10000** residual per missile wave.
    /// Fail-closed: not full ThingFactory Object / partition GeometryInfo matrix.
    pub fn honesty_scud_geometry_ok(&self) -> bool {
        self.strikes.values().any(|s| {
            s.kind == HostSuperweaponKind::ScudStorm
                && s.scud_geometry_applications > 0
                && s.scud_geometry_applications >= s.scud_ballistic_flight_applications
        }) && SCUD_STORM_MISSILE_GEOMETRY == "Cylinder"
            && SCUD_STORM_MISSILE_GEOMETRY_IS_SMALL
            && (SCUD_STORM_MISSILE_GEOMETRY_RADIUS - 7.0).abs() < 0.01
            && (SCUD_STORM_MISSILE_GEOMETRY_HEIGHT - 30.0).abs() < 0.01
            && (SCUD_STORM_MISSILE_MASS - 500.0).abs() < 0.01
            && (SCUD_STORM_MISSILE_MAX_HEALTH - 10000.0).abs() < 0.01
    }

    /// Residual honesty: SpectreHowitzerShell loft flight residual.
    ///
    /// Tracks pad-safe HeightDie InitialDelay loft sample + ground impact.
    /// Fail-closed: not full DumbProjectileBehavior Object / live Physics.
    /// Residual honesty: ScudStormMissile VisionRange / KindOf / Armor residual.
    ///
    /// Tracks VisionRange **300**, ShroudClearingRange **0**, KindOf PROJECTILE,
    /// Armor ProjectileArmor, TransportSlotCount **10**. Fail-closed: not full
    /// ThingFactory Object / partition KindOf matrix.
    pub fn honesty_scud_object_params_ok(&self) -> bool {
        self.strikes.values().any(|s| {
            s.kind == HostSuperweaponKind::ScudStorm
                && s.scud_object_params_applications > 0
                && s.scud_object_params_applications >= s.scud_geometry_applications
        }) && (SCUD_STORM_MISSILE_VISION_RANGE - 300.0).abs() < 0.01
            && (SCUD_STORM_MISSILE_SHROUD_CLEARING_RANGE - 0.0).abs() < 0.01
            && SCUD_STORM_MISSILE_KIND_OF == "PROJECTILE"
            && SCUD_STORM_MISSILE_ARMOR == "ProjectileArmor"
            && SCUD_STORM_MISSILE_TRANSPORT_SLOT_COUNT == 10
    }

    /// Residual honesty: ScudStormMissile MissileAIUpdate residual.
    ///
    /// Tracks TryToFollowTarget **No**, FuelLifetime **0**, InitialVelocity **0**,
    /// DistanceToTravelBeforeTurning **500**, DistanceToTargetBeforeDiving **200**,
    /// IgnitionFX residual. Fail-closed: not full live MissileAIUpdate physics.
    pub fn honesty_scud_missile_ai_ok(&self) -> bool {
        self.strikes.values().any(|s| {
            s.kind == HostSuperweaponKind::ScudStorm
                && s.scud_missile_ai_applications > 0
                && s.scud_missile_ai_applications >= s.scud_object_params_applications
        }) && !SCUD_STORM_MISSILE_TRY_FOLLOW_TARGET
            && SCUD_STORM_MISSILE_FUEL_LIFETIME == 0
            && (SCUD_STORM_MISSILE_INITIAL_VELOCITY - 0.0).abs() < 0.01
            && (SCUD_STORM_MISSILE_DISTANCE_BEFORE_TURNING - 500.0).abs() < 0.01
            && (SCUD_STORM_MISSILE_DISTANCE_BEFORE_DIVING - 200.0).abs() < 0.01
            && SCUD_STORM_MISSILE_IGNITION_FX == "FX_ScudStormIgnition"
    }

    /// Residual honesty: ScudStormMissile FireWeaponWhenDead death-weapon residual.
    ///
    /// Tracks base DeathWeapon ScudStormDamageWeapon (StartsActive Yes, ConflictsWith
    /// AnthraxBeta) + upgraded DeathWeapon ScudStormDamageWeaponUpgraded (StartsActive
    /// No, TriggeredBy AnthraxBeta). Fail-closed: not full FireWeaponWhenDeadBehavior
    /// exclusive module matrix / live upgrade toggle.
    pub fn honesty_scud_fire_weapon_when_dead_ok(&self) -> bool {
        self.strikes.values().any(|s| {
            s.kind == HostSuperweaponKind::ScudStorm
                && s.scud_fire_weapon_when_dead_applications > 0
        }) && SCUD_STORM_MISSILE_DEATH_WEAPON_BASE == "ScudStormDamageWeapon"
            && SCUD_STORM_MISSILE_DEATH_WEAPON_UPGRADED == "ScudStormDamageWeaponUpgraded"
            && SCUD_STORM_MISSILE_DEATH_CONFLICTS_WITH == "Upgrade_GLAAnthraxBeta"
            && SCUD_STORM_MISSILE_DEATH_TRIGGERED_BY == "Upgrade_GLAAnthraxBeta"
            && SCUD_STORM_MISSILE_DEATH_BASE_STARTS_ACTIVE
            && !SCUD_STORM_MISSILE_DEATH_UPGRADED_STARTS_ACTIVE
    }

    /// Residual honesty: ScudStormMissile body/draw residual params.
    ///
    /// Tracks InitialHealth **10000**, EditorSorting **SYSTEM**, OkToChangeModelColor
    /// Yes, DAMAGED model **NONE**. Fail-closed: not full ActiveBody / W3D ModelDraw.
    pub fn honesty_scud_body_draw_params_ok(&self) -> bool {
        self.strikes.values().any(|s| {
            s.kind == HostSuperweaponKind::ScudStorm && s.scud_body_draw_params_applications > 0
        }) && (SCUD_STORM_MISSILE_INITIAL_HEALTH - 10000.0).abs() < 0.01
            && (SCUD_STORM_MISSILE_INITIAL_HEALTH - SCUD_STORM_MISSILE_MAX_HEALTH).abs() < 0.01
            && SCUD_STORM_MISSILE_EDITOR_SORTING == "SYSTEM"
            && SCUD_STORM_MISSILE_OK_TO_CHANGE_MODEL_COLOR
            && SCUD_STORM_MISSILE_DAMAGED_MODEL == "NONE"
    }

    /// Residual honesty: SCUDStormMissileLocomotor Appearance residual.
    ///
    /// Tracks Surfaces **AIR**, Appearance **THRUST**, AllowAirborneMotiveForce Yes,
    /// Braking **0**. Fail-closed: not full Locomotor physics motive force matrix.
    pub fn honesty_scud_locomotor_appearance_ok(&self) -> bool {
        self.strikes.values().any(|s| {
            s.kind == HostSuperweaponKind::ScudStorm && s.scud_locomotor_appearance_applications > 0
        }) && SCUD_STORM_MISSILE_LOCOMOTOR_SURFACES == "AIR"
            && SCUD_STORM_MISSILE_LOCOMOTOR_APPEARANCE == "THRUST"
            && SCUD_STORM_MISSILE_LOCOMOTOR_ALLOW_AIRBORNE_MOTIVE
            && (SCUD_STORM_MISSILE_LOCOMOTOR_BRAKING - 0.0).abs() < 0.01
    }

    /// Residual honesty: ScudStormMissile DestroyDie + Locomotor name + Armor DamageFX.
    ///
    /// Tracks empty DestroyDie module presence, Locomotor template name
    /// **SCUDStormMissileLocomotor**, Armor DamageFX **None**. Fail-closed: not
    /// full DestroyDie Object / Locomotor store matrix / DamageFX module path.
    pub fn honesty_scud_destroy_die_locomotor_name_ok(&self) -> bool {
        self.strikes.values().any(|s| {
            s.kind == HostSuperweaponKind::ScudStorm
                && s.scud_destroy_die_locomotor_name_applications > 0
        }) && SCUD_STORM_MISSILE_DESTROY_DIE
            && SCUD_STORM_MISSILE_LOCOMOTOR_NAME == "SCUDStormMissileLocomotor"
            && SCUD_STORM_MISSILE_DAMAGE_FX == "None"
    }

    /// Wave 74 residual honesty: ScudStormMissile ThingFactory spawn bookkeeping.
    ///
    /// Tracks impact-time object pack ledger residual applications (not full
    /// ThingFactory Object / live MissileAIUpdate physics flight).
    pub fn honesty_scud_thing_factory_spawn_ok(&self) -> bool {
        self.strikes.values().any(|s| {
            s.kind == HostSuperweaponKind::ScudStorm
                && s.scud_thing_factory_spawn_applications > 0
                && s.scud_thing_factory_spawn_applications >= s.scud_object_params_applications
        }) && honesty_thing_factory_spawn_bookkeeping_wave74()
            && honesty_scud_storm_missile_thing_factory_pack()
    }

    /// Wave 74 residual honesty: SpectreHowitzerShell ThingFactory spawn bookkeeping.
    pub fn honesty_howitzer_shell_thing_factory_spawn_ok(&self) -> bool {
        self.orbit_fields.iter().any(|f| {
            f.howitzer_shell_thing_factory_spawn_applications > 0
                && f.howitzer_shell_thing_factory_spawn_applications >= f.howitzer_shells_spawned
        }) && honesty_spectre_howitzer_shell_thing_factory_pack()
    }

    /// Wave 74 residual honesty: TrailRemnant ThingFactory spawn bookkeeping.
    pub fn honesty_remnant_thing_factory_spawn_ok(&self) -> bool {
        self.remnant_fields.iter().any(|f| {
            f.remnant_thing_factory_spawn_applications >= 1
                && f.remnant_immortal_body_applications >= 1
                && f.remnant_fire_deletion_applications >= 1
        }) && honesty_trail_remnant_thing_factory_pack()
    }

    /// Residual honesty: Scud DeathWeapon FireOCL PoisonField residual.
    ///
    /// Tracks FireOCL **OCL_PoisonFieldLarge** (base) / **OCL_PoisonFieldUpgradedLarge**
    /// (AnthraxBeta). Fail-closed: not full FireWeaponWhenDead OCL spawn Object.
    pub fn honesty_scud_death_fire_ocl_ok(&self) -> bool {
        self.strikes.values().any(|s| {
            s.kind == HostSuperweaponKind::ScudStorm && s.scud_death_fire_ocl_applications > 0
        }) && SCUD_STORM_MISSILE_DEATH_FIRE_OCL_BASE == "OCL_PoisonFieldLarge"
            && SCUD_STORM_MISSILE_DEATH_FIRE_OCL_UPGRADED == "OCL_PoisonFieldUpgradedLarge"
    }

    /// Residual honesty: Scud Locomotor SpeedDamaged/MinSpeed/MaxThrustAngle residual.
    ///
    /// Tracks SpeedDamaged **200**, MinSpeed **100**, MaxThrustAngle **45**.
    /// Fail-closed: not full Locomotor thrust motive force matrix.
    pub fn honesty_scud_locomotor_speed_table_ok(&self) -> bool {
        self.strikes.values().any(|s| {
            s.kind == HostSuperweaponKind::ScudStorm
                && s.scud_locomotor_speed_table_applications > 0
        }) && (SCUD_STORM_MISSILE_LOCOMOTOR_SPEED_DAMAGED - 200.0).abs() < 0.01
            && (SCUD_STORM_MISSILE_LOCOMOTOR_MIN_SPEED - 100.0).abs() < 0.01
            && (SCUD_STORM_MISSILE_LOCOMOTOR_MAX_THRUST_ANGLE - 45.0).abs() < 0.01
            && (SCUD_STORM_MISSILE_LOCOMOTOR_SPEED - 300.0).abs() < 0.01
            && (SCUD_STORM_MISSILE_LOCOMOTOR_ACCEL - 675.0).abs() < 0.01
            && (SCUD_STORM_MISSILE_LOCOMOTOR_TURN_RATE - 540.0).abs() < 0.01
    }

    /// Residual honesty: Scud DeathWeapon Primary/Secondary damage table residual.
    ///
    /// Tracks PrimaryDamage **500**, PrimaryDamageRadius **50**, SecondaryDamage
    /// **150**/**200** (upgraded), SecondaryDamageRadius **200**, DamageType
    /// **EXPLOSION**, DeathType **EXPLODED**, WeaponSpeed **600**, AttackRange **200**,
    /// FireFX **ScudStormMissileDetonation**, RadiusDamageAffects ALLIES/ENEMIES/NEUTRALS.
    /// Fail-closed: not full FireWeaponWhenDeadBehavior exclusive module matrix.
    pub fn honesty_scud_death_damage_table_ok(&self) -> bool {
        self.strikes.values().any(|s| {
            s.kind == HostSuperweaponKind::ScudStorm && s.scud_death_damage_table_applications > 0
        }) && (SCUD_STORM_PRIMARY_DAMAGE - 500.0).abs() < 0.01
            && (SCUD_STORM_PRIMARY_RADIUS - 50.0).abs() < 0.01
            && (SCUD_STORM_SECONDARY_DAMAGE - 150.0).abs() < 0.01
            && (SCUD_STORM_SECONDARY_DAMAGE_UPGRADED - 200.0).abs() < 0.01
            && (SCUD_STORM_SECONDARY_RADIUS - 200.0).abs() < 0.01
            && SCUD_STORM_MISSILE_DEATH_DAMAGE_TYPE == "EXPLOSION"
            && SCUD_STORM_MISSILE_DEATH_DEATH_TYPE == "EXPLODED"
            && (SCUD_STORM_MISSILE_DEATH_WEAPON_SPEED - 600.0).abs() < 0.01
            && (SCUD_STORM_MISSILE_DEATH_ATTACK_RANGE - 200.0).abs() < 0.01
            && SCUD_STORM_MISSILE_DEATH_FIRE_FX == "ScudStormMissileDetonation"
            && SCUD_STORM_MISSILE_DEATH_RADIUS_DAMAGE_AFFECTS == "ALLIES ENEMIES NEUTRALS"
            && SCUD_STORM_MISSILE_DEATH_DELAY_BETWEEN_SHOTS_MS == 0
            && SCUD_STORM_MISSILE_DEATH_CLIP_SIZE == 0
            && SCUD_STORM_MISSILE_DEATH_CLIP_RELOAD_TIME_MS == 0
    }

    /// Residual honesty: SpectreHowitzerShellLocomotor template residual.
    ///
    /// Tracks Surfaces **AIR**, Appearance **THRUST**, MinSpeed **1111**, Accel
    /// **9160**, TurnRate **99999**, MaxThrustAngle **90**, Braking **0**,
    /// AllowAirborneMotiveForce Yes. Fail-closed: not full Locomotor store /
    /// live motive force (Object comments out Locomotor when DumbProjectile active).
    pub fn honesty_howitzer_shell_locomotor_template_ok(&self) -> bool {
        self.orbit_fields.iter().any(|f| {
            f.howitzer_shell_locomotor_template_applications > 0
                && f.howitzer_shell_locomotor_template_applications >= f.howitzer_shells_spawned
        }) && SPECTRE_HOWITZER_SHELL_LOCOMOTOR_NAME == "SpectreHowitzerShellLocomotor"
            && SPECTRE_HOWITZER_SHELL_LOCOMOTOR_SURFACES == "AIR"
            && SPECTRE_HOWITZER_SHELL_LOCOMOTOR_APPEARANCE == "THRUST"
            && (SPECTRE_HOWITZER_SHELL_LOCOMOTOR_MIN_SPEED - 1111.0).abs() < 0.01
            && (SPECTRE_HOWITZER_SHELL_LOCOMOTOR_SPEED - 1111.0).abs() < 0.01
            && (SPECTRE_HOWITZER_SHELL_LOCOMOTOR_ACCEL - 9160.0).abs() < 0.01
            && (SPECTRE_HOWITZER_SHELL_LOCOMOTOR_TURN_RATE - 99999.0).abs() < 0.01
            && (SPECTRE_HOWITZER_SHELL_LOCOMOTOR_MAX_THRUST_ANGLE - 90.0).abs() < 0.01
            && (SPECTRE_HOWITZER_SHELL_LOCOMOTOR_BRAKING - 0.0).abs() < 0.01
            && SPECTRE_HOWITZER_SHELL_LOCOMOTOR_ALLOW_AIRBORNE
    }

    /// Residual honesty: SpectreHowitzerShell Armor DamageFX residual.
    ///
    /// Tracks ArmorSet DamageFX **None**. Fail-closed: not full DamageFXStore path.
    pub fn honesty_howitzer_shell_damage_fx_ok(&self) -> bool {
        self.orbit_fields.iter().any(|f| {
            f.howitzer_shell_damage_fx_applications > 0
                && f.howitzer_shell_damage_fx_applications >= f.howitzer_shells_spawned
        }) && SPECTRE_HOWITZER_SHELL_DAMAGE_FX == "None"
    }

    /// Residual honesty: SpectreHowitzerGun AcceptableAimDelta / AttackRange residual.
    ///
    /// Tracks AcceptableAimDelta **180**, AttackRange **2222**, ProjectileCollidesWith
    /// **STRUCTURES WALLS**, AntiGround **Yes**. Fail-closed: not full WeaponTemplate
    /// store / live turret aim matrix.
    pub fn honesty_howitzer_gun_aim_params_ok(&self) -> bool {
        self.orbit_fields.iter().any(|f| {
            f.howitzer_gun_aim_params_applications > 0
                && f.howitzer_gun_aim_params_applications >= f.howitzer_shells_spawned
        }) && (SPECTRE_HOWITZER_ACCEPTABLE_AIM_DELTA - 180.0).abs() < 0.01
            && (SPECTRE_HOWITZER_ATTACK_RANGE - 2222.0).abs() < 0.01
            && SPECTRE_HOWITZER_PROJECTILE_COLLIDES_WITH == "STRUCTURES WALLS"
            && SPECTRE_HOWITZER_ANTI_GROUND
            && (SPECTRE_HOWITZER_WEAPON_SPEED - 999.0).abs() < 0.01
    }

    /// Residual honesty: SpectreHowitzerGun fire residual.
    ///
    /// Tracks PrimaryDamage **80**, PrimaryDamageRadius **25**, DelayBetweenShots
    /// **777** ms, DamageType **EXPLOSION**, DeathType **EXPLODED**,
    /// RadiusDamageAffects **ALLIES ENEMIES NEUTRALS**, FireFX/FireSound/DetonationFX,
    /// ClipSize **0**. Fail-closed: not full WeaponTemplate store / live turret fire matrix.
    pub fn honesty_howitzer_gun_fire_params_ok(&self) -> bool {
        self.orbit_fields.iter().any(|f| {
            f.howitzer_gun_fire_params_applications > 0
                && f.howitzer_gun_fire_params_applications >= f.howitzer_shells_spawned
        }) && (SPECTRE_HOWITZER_PRIMARY_DAMAGE - 80.0).abs() < 0.01
            && (SPECTRE_HOWITZER_RADIUS - 25.0).abs() < 0.01
            && SPECTRE_HOWITZER_DELAY_BETWEEN_SHOTS_MS == 777
            && SPECTRE_HOWITZER_DELAY_BETWEEN_SHOTS_FRAMES == 23
            && SPECTRE_HOWITZER_DAMAGE_TYPE == "EXPLOSION"
            && SPECTRE_HOWITZER_DEATH_TYPE == "EXPLODED"
            && SPECTRE_HOWITZER_RADIUS_DAMAGE_AFFECTS == "ALLIES ENEMIES NEUTRALS"
            && SPECTRE_HOWITZER_FIRE_FX.contains("GenericTankGunNoTracer")
            && SPECTRE_HOWITZER_FIRE_SOUND.contains("ArtilleryRound")
            && SPECTRE_HOWITZER_DETONATION_FX.contains("SpectreHowitzerExplosion")
            && SPECTRE_HOWITZER_CLIP_SIZE == 0
            && SPECTRE_HOWITZER_CLIP_RELOAD_TIME_MS == 0
            && SPECTRE_HOWITZER_SHELL_LOCOMOTOR_GROUP_PRIORITY == "MOVES_BACK"
    }

    /// Residual honesty: ScudStormWeapon launch residual.
    ///
    /// Tracks ClipSize **9**, ClipReloadTime **10000** ms, AutoReloadsClip **Yes**,
    /// ScatterTargetScalar **120**, ScatterTarget count **9**, AcceptableAimDelta
    /// **180**, ProjectileCollidesWith **STRUCTURES**, DelayBetweenShots Min/Max
    /// **100**/**1000** ms, ProjectileObject **ScudStormMissile**, Death ClipReloadTime
    /// **0**. Fail-closed: not full WeaponTemplate store / live pad reload matrix.
    pub fn honesty_scud_weapon_launch_ok(&self) -> bool {
        self.strikes.values().any(|s| {
            s.kind == HostSuperweaponKind::ScudStorm && s.scud_weapon_launch_applications > 0
        }) && SCUD_STORM_CLIP_SIZE == 9
            && SCUD_STORM_CLIP_SIZE == SCUD_STORM_MISSILE_COUNT
            && SCUD_STORM_CLIP_RELOAD_TIME_MS == 10000
            && SCUD_STORM_CLIP_RELOAD_FRAMES == 300
            && SCUD_STORM_AUTO_RELOADS_CLIP
            && (SCUD_STORM_SCATTER_SCALAR - 120.0).abs() < 0.01
            && SCUD_STORM_SCATTER_TARGET_COUNT == 9
            && SCUD_STORM_SCATTER_TARGETS.len() as u32 == SCUD_STORM_SCATTER_TARGET_COUNT
            && (SCUD_STORM_ACCEPTABLE_AIM_DELTA - 180.0).abs() < 0.01
            && SCUD_STORM_PROJECTILE_COLLIDES_WITH == "STRUCTURES"
            && SCUD_STORM_PROJECTILE_OBJECT == "ScudStormMissile"
            && SCUD_STORM_DELAY_BETWEEN_MIN_MS == 100
            && SCUD_STORM_DELAY_BETWEEN_MAX_MS == 1000
            && SCUD_STORM_DELAY_BETWEEN_MIN_FRAMES == 3
            && SCUD_STORM_DELAY_BETWEEN_MAX_FRAMES == 30
            && SCUD_STORM_MISSILE_DEATH_CLIP_RELOAD_TIME_MS == 0
            && SCUD_STORM_MISSILE_DEATH_CLIP_SIZE == 0
    }

    /// Residual honesty: SpectreHowitzerGun anti residual.
    ///
    /// Tracks AntiAirborneVehicle/Infantry **No**, AntiSmallMissile/AntiBallisticMissile
    /// **No**, ProjectileObject **SpectreHowitzerShell**, ContinuousFireCoast **2000** ms,
    /// ContinuousFireOne/Two **1**/**2**, VeterancyFireFX residual.
    /// Fail-closed: not full WeaponTemplate anti matrix / live turret aim.
    pub fn honesty_howitzer_gun_anti_params_ok(&self) -> bool {
        self.orbit_fields.iter().any(|f| {
            f.howitzer_gun_anti_params_applications > 0
                && f.howitzer_gun_anti_params_applications >= f.howitzer_shells_spawned
        }) && !SPECTRE_HOWITZER_ANTI_AIRBORNE_VEHICLE
            && !SPECTRE_HOWITZER_ANTI_AIRBORNE_INFANTRY
            && !SPECTRE_HOWITZER_ANTI_SMALL_MISSILE
            && !SPECTRE_HOWITZER_ANTI_BALLISTIC_MISSILE
            && SPECTRE_HOWITZER_ANTI_GROUND
            && SPECTRE_HOWITZER_PROJECTILE_OBJECT == "SpectreHowitzerShell"
            && SPECTRE_HOWITZER_PROJECTILE_OBJECT == SPECTRE_HOWITZER_SHELL_OBJECT
            && SPECTRE_HOWITZER_CONTINUOUS_FIRE_COAST_MS == 2000
            && SPECTRE_CONTINUOUS_FIRE_COAST_FRAMES == 60
            && SPECTRE_HOWITZER_CONTINUOUS_FIRE_ONE == 1
            && SPECTRE_HOWITZER_CONTINUOUS_FIRE_TWO == 2
            && SPECTRE_HOWITZER_VETERANCY_FIRE_FX.contains("GenericTankGunNoTracer")
            && SPECTRE_HOWITZER_FIRE_FX.contains("GenericTankGunNoTracer")
    }

    /// Residual honesty: ScudStormWeapon special residual (unused combat fields).
    ///
    /// Tracks PrimaryDamage **0**, PrimaryDamageRadius **0**, AttackRange **999999**,
    /// DamageType **EXPLOSION**, DeathType **EXPLODED**, WeaponSpeed **99999**,
    /// ScatterRadius **0**, PreAttackType **PER_CLIP**, PreAttackDelay **3000** ms.
    /// Fail-closed: not full WeaponTemplate store / live pad launch matrix.
    pub fn honesty_scud_weapon_special_ok(&self) -> bool {
        self.strikes.values().any(|s| {
            s.kind == HostSuperweaponKind::ScudStorm && s.scud_weapon_special_applications > 0
        }) && (SCUD_STORM_WEAPON_PRIMARY_DAMAGE - 0.0).abs() < 0.01
            && (SCUD_STORM_WEAPON_PRIMARY_RADIUS - 0.0).abs() < 0.01
            && (SCUD_STORM_WEAPON_ATTACK_RANGE - 999_999.0).abs() < 0.01
            && SCUD_STORM_WEAPON_DAMAGE_TYPE == "EXPLOSION"
            && SCUD_STORM_WEAPON_DEATH_TYPE == "EXPLODED"
            && (SCUD_STORM_WEAPON_SPEED - 99_999.0).abs() < 0.01
            && (SCUD_STORM_SCATTER_RADIUS - 0.0).abs() < 0.01
            && SCUD_STORM_PRE_ATTACK_TYPE == "PER_CLIP"
            && SCUD_STORM_PRE_ATTACK_DELAY_MS == 3000
            && SCUD_STORM_PRE_ATTACK_FRAMES == 90
            && SCUD_STORM_PRE_ATTACK_FRAMES == (SCUD_STORM_PRE_ATTACK_DELAY_MS * 30) / 1000
            && SCUD_STORM_PROJECTILE_DETONATION_FX == "ScudStormMissileDetonation"
            && SCUD_STORM_WEAPON_RADIUS_DAMAGE_AFFECTS == "ALLIES ENEMIES NEUTRALS"
            && SCUD_STORM_FIRE_FX == "WeaponFX_ScudStormMissile"
            && SCUD_STORM_MISSILE_LAUNCH_SOUND == "ScudStormLaunch"
            && SCUD_STORM_MISSILE_EXHAUST == "ScudMissileExhaust"
    }

    /// Residual honesty: Scud MissileAIUpdate defaults residual.
    ///
    /// Tracks IgnitionDelay **0**, UseWeaponSpeed **No**, DetonateOnNoFuel **No**,
    /// DistanceToTargetForLock **75**, DistanceScatterWhenJammed **75**,
    /// DetonateCallsKill **No**, KillSelfDelay **3** frames (C++ module defaults
    /// not overridden in ScudStormMissile INI). Fail-closed: not full MissileAIUpdate
    /// state machine / live fuel/jam/kill-self path.
    pub fn honesty_scud_missile_ai_defaults_ok(&self) -> bool {
        self.strikes.values().any(|s| {
            s.kind == HostSuperweaponKind::ScudStorm && s.scud_missile_ai_defaults_applications > 0
        }) && SCUD_STORM_MISSILE_IGNITION_DELAY_FRAMES == 0
            && !SCUD_STORM_MISSILE_USE_WEAPON_SPEED
            && !SCUD_STORM_MISSILE_DETONATE_ON_NO_FUEL
            && (SCUD_STORM_MISSILE_DISTANCE_FOR_LOCK - 75.0).abs() < 0.01
            && (SCUD_STORM_MISSILE_DISTANCE_SCATTER_WHEN_JAMMED - 75.0).abs() < 0.01
            && !SCUD_STORM_MISSILE_DETONATE_CALLS_KILL
            && SCUD_STORM_MISSILE_KILL_SELF_DELAY_FRAMES == 3
            && !SCUD_STORM_MISSILE_TRY_FOLLOW_TARGET
            && SCUD_STORM_MISSILE_FUEL_LIFETIME == 0
    }

    /// Residual honesty: SpectreGattlingGun anti/fire residual.
    ///
    /// Tracks AntiAirborne*/AntiMissile **No**, AntiGround **Yes**, ProjectileObject
    /// **NONE**, PrimaryDamageRadius **0**, DamageType **Gattling**, DeathType
    /// **NORMAL**, WeaponSpeed **999999**, AttackRange **2222**, ClipSize **0**,
    /// FireFX/VeterancyFireFX residual. Fail-closed: not full WeaponTemplate
    /// anti matrix / live hitscan aim.
    pub fn honesty_gattling_gun_params_ok(&self) -> bool {
        self.orbit_fields.iter().any(|f| {
            f.gattling_gun_params_applications > 0
                && f.gattling_gun_params_applications >= f.gattling_ticks
        }) && !SPECTRE_GATTLING_ANTI_AIRBORNE_VEHICLE
            && !SPECTRE_GATTLING_ANTI_AIRBORNE_INFANTRY
            && !SPECTRE_GATTLING_ANTI_SMALL_MISSILE
            && !SPECTRE_GATTLING_ANTI_BALLISTIC_MISSILE
            && SPECTRE_GATTLING_ANTI_GROUND
            && SPECTRE_GATTLING_PROJECTILE_OBJECT == "NONE"
            && (SPECTRE_GATTLING_PRIMARY_RADIUS - 0.0).abs() < 0.01
            && (SPECTRE_GATTLING_DAMAGE - 90.0).abs() < 0.01
            && (SPECTRE_GATTLING_ATTACK_RANGE - 2222.0).abs() < 0.01
            && SPECTRE_GATTLING_DAMAGE_TYPE == "Gattling"
            && SPECTRE_GATTLING_DEATH_TYPE == "NORMAL"
            && (SPECTRE_GATTLING_WEAPON_SPEED - 999_999.0).abs() < 0.01
            && SPECTRE_GATTLING_FIRE_FX.contains("SpectreGattlingMuzzleFlash")
            && SPECTRE_GATTLING_VETERANCY_FIRE_FX.contains("RedTracers")
            && SPECTRE_GATTLING_RADIUS_DAMAGE_AFFECTS == "ALLIES ENEMIES NEUTRALS"
            && SPECTRE_GATTLING_DELAY_BETWEEN_SHOTS_MS == 100
            && SPECTRE_GATTLING_TICK_INTERVAL_FRAMES == 3
            && SPECTRE_GATTLING_CLIP_SIZE == 0
            && SPECTRE_GATTLING_CLIP_RELOAD_TIME_MS == 0
    }

    /// Residual honesty: connector KindOf IMMOBILE + Segments/MaxIntensity/Fade/Tile.
    ///
    /// Tracks KindOf **IMMOBILE**, Segments **1**, MaxIntensityLifetime **0**,
    /// FadeLifetime **0**, Tile **No** residual defaults for Medium/Intense connectors.
    /// Fail-closed: not full LaserUpdate GPU drawable / ThingFactory connector Object.
    pub fn honesty_beam_connector_kindof_defaults_ok(&self) -> bool {
        self.beam_fields.iter().any(|f| {
            f.connector_kindof_immobile_armed >= 1
                && f.connector_segments_armed == PARTICLE_CONNECTOR_SEGMENTS
                && f.connector_max_intensity_fade_armed >= 1
                && f.connector_tile_no_armed >= 1
        }) && PARTICLE_CONNECTOR_KIND_OF == "IMMOBILE"
            && PARTICLE_CONNECTOR_SEGMENTS == 1
            && (PARTICLE_CONNECTOR_ARC_HEIGHT - 0.0).abs() < 0.01
            && (PARTICLE_CONNECTOR_SEGMENT_OVERLAP - 0.0).abs() < 0.01
            && PARTICLE_CONNECTOR_MAX_INTENSITY_FRAMES == 0
            && PARTICLE_CONNECTOR_FADE_FRAMES == 0
            && !PARTICLE_CONNECTOR_TILE
    }

    /// Residual honesty: TrailRemnant KindOf + ImmortalBody residual.
    ///
    /// Tracks KindOf **NO_COLLIDE UNATTACKABLE IMMOBILE**, ImmortalBody MaxHealth
    /// **50**, InitialHealth **50**, EditorSorting **SYSTEM**. Fail-closed: not
    /// full ThingFactory Object / ImmortalBody / DeletionUpdate module stack.
    pub fn honesty_beam_remnant_object_params_ok(&self) -> bool {
        self.remnant_fields
            .iter()
            .any(|f| f.remnant_object_params_applications >= 1)
            && PARTICLE_REMNANT_KIND_OF == "NO_COLLIDE UNATTACKABLE IMMOBILE"
            && (PARTICLE_REMNANT_MAX_HEALTH - 50.0).abs() < 0.01
            && (PARTICLE_REMNANT_INITIAL_HEALTH - 50.0).abs() < 0.01
            && (PARTICLE_REMNANT_INITIAL_HEALTH - PARTICLE_REMNANT_MAX_HEALTH).abs() < 0.01
            && PARTICLE_REMNANT_EDITOR_SORTING == "SYSTEM"
            && PARTICLE_REMNANT_BODY == "ImmortalBody"
            && PARTICLE_REMNANT_OBJECT_NAME == "ParticleUplinkCannonTrailRemnant"
    }

    /// Residual honesty: TrailRemnant FireWeaponUpdate + DeletionUpdate residual.
    ///
    /// Tracks FireWeaponUpdate Weapon **ParticleUplinkCannonBeamTrailRemnantWeapon**,
    /// PrimaryDamage **15** / radius **10** / DelayBetweenShots **250** ms,
    /// DamageType **PARTICLE_BEAM**, DeathType **BURNED**, DeletionUpdate Min/Max
    /// Lifetime **4000** ms. Fail-closed: not full ThingFactory Object / live
    /// FireWeaponUpdate + DeletionUpdate module stack.
    pub fn honesty_beam_remnant_fire_deletion_ok(&self) -> bool {
        self.remnant_fields
            .iter()
            .any(|f| f.remnant_fire_deletion_applications >= 1)
            && PARTICLE_REMNANT_FIRE_WEAPON_UPDATE
            && PARTICLE_REMNANT_DELETION_UPDATE
            && PARTICLE_REMNANT_WEAPON_NAME == "ParticleUplinkCannonBeamTrailRemnantWeapon"
            && (PARTICLE_REMNANT_DAMAGE_PER_TICK - 15.0).abs() < 0.01
            && (PARTICLE_REMNANT_RADIUS - 10.0).abs() < 0.01
            && PARTICLE_REMNANT_TICK_INTERVAL_FRAMES == 7
            && PARTICLE_REMNANT_DURATION_FRAMES == 120
            && PARTICLE_REMNANT_MIN_LIFETIME_MS == 4000
            && PARTICLE_REMNANT_MAX_LIFETIME_MS == 4000
            && PARTICLE_REMNANT_MIN_LIFETIME_MS == PARTICLE_REMNANT_MAX_LIFETIME_MS
            && PARTICLE_REMNANT_DAMAGE_TYPE == "PARTICLE_BEAM"
            && PARTICLE_REMNANT_DEATH_TYPE == "BURNED"
            && PARTICLE_REMNANT_RADIUS_DAMAGE_AFFECTS == "ALLIES ENEMIES NEUTRALS"
            && (PARTICLE_REMNANT_WEAPON_SPEED - 250.0).abs() < 0.01
    }

    /// Residual honesty: TrailRemnant ImmortalBody health-floor residual.
    ///
    /// Tracks ImmortalBody floor **1** HP (`internalChangeHealth` clamp) and
    /// never-dead residual. Fail-closed: not full ActiveBody / Object death flag
    /// / ThingFactory ImmortalBody module stack.
    pub fn honesty_beam_remnant_immortal_body_ok(&self) -> bool {
        self.remnant_fields
            .iter()
            .any(|f| f.remnant_immortal_body_applications >= 1)
            && (PARTICLE_REMNANT_IMMORTAL_HEALTH_FLOOR - 1.0).abs() < 0.01
            && PARTICLE_REMNANT_IMMORTAL_NEVER_DEAD
            && PARTICLE_REMNANT_BODY == "ImmortalBody"
            && (PARTICLE_REMNANT_MAX_HEALTH - 50.0).abs() < 0.01
            && honesty_immortal_body_health_floor(50.0, -100.0, 1.0)
            && honesty_immortal_body_health_floor(50.0, -10.0, 40.0)
            && honesty_immortal_body_health_floor(1.0, -5.0, 1.0)
            && honesty_immortal_body_health_floor(10.0, 5.0, 15.0)
    }

    pub fn honesty_howitzer_shell_loft_flight_ok(&self) -> bool {
        self.orbit_fields.iter().any(|f| {
            f.howitzer_shell_loft_flight_applications > 0
                && f.howitzer_shell_loft_height_die_applications > 0
        }) && SPECTRE_HOWITZER_HEIGHT_DIE_INITIAL_DELAY_FRAMES == 30
            && SPECTRE_HOWITZER_SHELL_HEIGHT_DIE_ONLY_MOVING_DOWN
            && (SPECTRE_HOWITZER_WEAPON_SPEED - 999.0).abs() < 0.01
    }

    /// Residual honesty: LaserUpdate client residual (ground-to-orbit / orbit-to-target).
    ///
    /// Tracks initLaser start/end, drawable midpoint, WidthGrow sizeDelta widen
    /// scalar, dirty residual, and orbit altitude **500**. Fail-closed: not full
    /// LaserUpdate drawable matrix / client shroud / GPU SegLine submit.
    pub fn honesty_beam_laser_update_ok(&self) -> bool {
        self.beam_fields.iter().any(|f| {
            f.laser_update_init_applications >= 1
                && f.laser_update_dirty
                && f.laser_update_growth_frames == PARTICLE_WIDTH_GROW_FRAMES
                && f.last_laser_update_end != Vec3::ZERO
                && f.last_laser_update_drawable_mid != Vec3::ZERO
        }) && (PARTICLE_LASER_ORBIT_ALTITUDE - 500.0).abs() < 0.01
            && PARTICLE_WIDTH_GROW_FRAMES == 60
            && (laser_update_width_scalar_widen(0, PARTICLE_WIDTH_GROW_FRAMES) - 0.0).abs() < 0.01
            && (laser_update_width_scalar_widen(
                PARTICLE_WIDTH_GROW_FRAMES,
                PARTICLE_WIDTH_GROW_FRAMES,
            ) - 1.0)
                .abs()
                < 0.01
            && (laser_update_width_scalar_decay(0, PARTICLE_WIDTH_GROW_FRAMES) - 1.0).abs() < 0.01
            && (laser_update_width_scalar_decay(
                PARTICLE_WIDTH_GROW_FRAMES,
                PARTICLE_WIDTH_GROW_FRAMES,
            ) - 0.0)
                .abs()
                < 0.01
    }

    /// Residual honesty: once-at-queue multi-strike OCL residual plan.
    ///
    /// True when a multi-strike Artillery/Carpet/Scud strike stored epicenters
    /// + shell frames at queue (retail once-at-create stream residual).
    /// Fail-closed: not live mid-sim global stream mutation / full transport Object.
    pub fn honesty_once_at_queue_ocl_ok(&self) -> bool {
        self.strikes.values().any(|s| {
            s.kind.is_multi_strike()
                && s.ocl_once_at_queue_armed >= 1
                && !s.ocl_points.is_empty()
                && s.ocl_shell_frames.len() == s.ocl_points.len()
                && s.ocl_shell_frames.first().copied().unwrap_or(0) >= s.impact_frame
        })
    }

    /// Advance ParticleCannon pre-fire intensity schedule + beam FIRING/POSTFIRE/
    /// PACKING intensity residual + BeamLaunchFX refresh + Scud PreAttack residual.
    ///
    /// Call once per logic frame (before impact planning is fine).
    pub fn advance_particle_intensity_schedule(&mut self, current_frame: u32) {
        // Pre-fire charge residual on queued ParticleCannon strikes.
        let particle_ids: Vec<u32> = self
            .strikes
            .values()
            .filter(|s| {
                s.kind == HostSuperweaponKind::ParticleCannon && s.phase == HostStrikePhase::Queued
            })
            .map(|s| s.id)
            .collect();
        let mut pending_puc_audio: Vec<(ObjectId, Vec3, &'static str)> = Vec::new();
        for id in particle_ids {
            if let Some(strike) = self.strikes.get_mut(&id) {
                if let Some(cue) = apply_particle_charge_status(strike, current_frame) {
                    pending_puc_audio.push((strike.source_object, strike.target_position, cue));
                }
            }
        }
        for (src, pos, cue) in pending_puc_audio {
            self.note_puc_loop_audio(src, pos, cue);
        }

        // ScudStorm PreAttack residual frame counter (until first missile wave).
        for strike in self.strikes.values_mut() {
            if strike.kind == HostSuperweaponKind::ScudStorm
                && strike.phase == HostStrikePhase::Queued
                && strike.scud_pre_attack_active
                && current_frame >= strike.activate_frame
                && current_frame < strike.impact_frame
            {
                strike.scud_pre_attack_frames = strike.scud_pre_attack_frames.saturating_add(1);
            }
        }

        // Beam attack-phase intensity residual (FIRING → POSTFIRE → PACKING).
        for field in &mut self.beam_fields {
            if field.is_expired(current_frame) && field.status != ParticleUplinkStatus::Packing {
                // Past orbital death: PACKING residual (effects cleared).
                if field.status != ParticleUplinkStatus::Packing {
                    field.intensity_transitions = field.intensity_transitions.saturating_add(1);
                }
                field.status = ParticleUplinkStatus::Packing;
                field.packing_applications = field.packing_applications.saturating_add(1);
                field.outer_intensity = ParticleIntensity::None;
                field.connector_intensity = ParticleIntensity::None;
                field.laser_base_intensity = ParticleIntensity::None;
                field.outer_node_systems_created = 0;
                field.connector_lasers_created = 0;
                field.laser_base_flare_created = 0;
                field.ground_to_orbit_laser_created = 0;
                field.connector_flare_created = 0;
                continue;
            }
            if field.is_expired(current_frame) {
                continue;
            }
            let firing_frames = field
                .live_decay_start_frame()
                .saturating_sub(field.spawn_frame);
            let next_status = particle_status_for_attack(
                current_frame,
                field.spawn_frame,
                firing_frames,
                PARTICLE_WIDTH_GROW_FRAMES,
            );
            if next_status != field.status {
                field.intensity_transitions = field.intensity_transitions.saturating_add(1);
                field.status = next_status;
                let fx = particle_client_effects_for_status(next_status);
                field.outer_node_systems_created = fx.outer_nodes;
                field.outer_intensity = fx.outer_intensity;
                field.connector_lasers_created = fx.connector_lasers;
                field.connector_intensity = fx.connector_intensity;
                field.connector_flare_created = fx.connector_flare;
                field.laser_base_flare_created = fx.laser_base;
                field.laser_base_intensity = fx.laser_base_intensity;
                field.ground_to_orbit_laser_created = fx.ground_to_orbit;
                let flare_origin = if field.source_axis_set {
                    field.source_position
                } else {
                    field.position
                };
                spawn_particle_outer_node_flares(
                    field.source_object,
                    flare_origin,
                    field.outer_intensity,
                );
                match next_status {
                    ParticleUplinkStatus::Postfire => {
                        field.postfire_applications = field.postfire_applications.saturating_add(1);
                        // Medium connector soft-edge residual (NumBeams 4, 0.4→1.2).
                        if field.connector_intensity == ParticleIntensity::Medium {
                            field.medium_connector_soft_edge_armed =
                                field.medium_connector_soft_edge_armed.saturating_add(1);
                            let peak = particle_connector_medium_soft_edge_width(
                                PARTICLE_CONNECTOR_MEDIUM_NUM_BEAMS.saturating_sub(1),
                            );
                            if peak > field.peak_medium_connector_soft_edge_outer_width {
                                field.peak_medium_connector_soft_edge_outer_width = peak;
                            }
                        }
                        // LaserUpdate setDecayFrames(WidthGrow) residual at POSTFIRE.
                        field.laser_update_decaying = true;
                        field.laser_update_widening = false;
                        field.laser_update_dirty = true;
                    }
                    ParticleUplinkStatus::Packing => {
                        field.packing_applications = field.packing_applications.saturating_add(1);
                    }
                    _ => {}
                }
            }
            // BeamLaunchFX residual refresh while STATUS_FIRING.
            if field.status == ParticleUplinkStatus::Firing
                && current_frame >= field.next_launch_fx_frame
            {
                field.beam_launch_fx_applications =
                    field.beam_launch_fx_applications.saturating_add(1);
                field.next_launch_fx_frame = current_frame
                    .saturating_add(PARTICLE_LAUNCH_FX_INTERVAL_FRAMES)
                    .max(field.next_launch_fx_frame.saturating_add(1));
                let origin = if field.source_axis_set {
                    field.source_position
                } else {
                    field.position
                };
                play_particle_beam_launch_fx(origin);
            }
        }
    }

    /// Residual honesty: TotalScorchMarks residual applied at least one mark.
    ///
    /// Wave 45: also requires ScorchMarkScalar **2.4** residual pack armed.
    pub fn honesty_beam_scorch_ok(&self) -> bool {
        (self.beam_fields.iter().any(|f| f.scorch_marks_made > 0)
            || self
                .beam_fields
                .iter()
                .any(|f| f.ground_hit_fx_applications > 0))
            && self
                .beam_fields
                .iter()
                .any(|f| f.scorch_scalar_pack_armed >= 1)
            && honesty_particle_scorch_pack()
    }

    /// Residual honesty: PUC sound residual pack applied on beam spawn / charge.
    ///
    /// Tracks PoweringUp / UnpackToIdle / FiringToPack / GroundAnnihilation
    /// names + BeamLaunchFX interval + GroundHitFX. Prefire UnpackToIdle arms
    /// on PREPARING (host impact_delay seeds PREPARING); PoweringUp arms when
    /// CHARGING window is reached. Fail-closed: not full Miles audio loops.
    pub fn honesty_beam_sound_residual_ok(&self) -> bool {
        let beam_ok = self.beam_fields.iter().any(|f| {
            f.ground_annihilation_audio_applications >= 1
                && f.firing_to_pack_audio_applications >= 1
                && f.sound_residual_pack_armed >= 1
                && f.beam_launch_fx_applications >= 1
        });
        let prefire_ok = self.strikes.values().any(|s| {
            s.kind == HostSuperweaponKind::ParticleCannon
                && s.particle_unpack_audio_applications > 0
        });
        beam_ok && prefire_ok && honesty_particle_sound_loops()
    }

    /// Residual honesty: PointDefense laser LifetimeUpdate residual constants.
    pub fn honesty_point_defense_laser_lifetime_ok(&self) -> bool {
        honesty_point_defense_laser_lifetime()
    }

    /// Residual honesty: PUC FlammableUpdate residual constants.
    pub fn honesty_particle_uplink_flammable_ok(&self) -> bool {
        honesty_particle_uplink_flammable()
    }

    /// Residual honesty: OuterNodes flare residual pack armed on beam spawn.
    ///
    /// Tracks Light/Medium/Intense outer-node flare names + LaserBaseReadyToFire
    /// + connector laser names. Fail-closed: not full ParticleSystemManager attach.
    pub fn honesty_beam_outer_node_flare_pack_ok(&self) -> bool {
        honesty_particle_outer_node_flare_pack()
            && self
                .beam_fields
                .iter()
                .any(|f| f.outer_node_flare_pack_armed >= 1)
    }

    /// Residual honesty: PUC SlowDeath / InstantDeath residual pack constants.
    ///
    /// When a beam field is present, also requires death_pack_armed. Pure constant
    /// pack remains honest without a live building Object die path.
    pub fn honesty_particle_uplink_death_pack_ok(&self) -> bool {
        let constants = honesty_particle_uplink_death_pack();
        if self.beam_fields.is_empty() {
            return constants;
        }
        constants && self.beam_fields.iter().any(|f| f.death_pack_armed >= 1)
    }

    /// Residual honesty: RevealRange residual applied at least once with scorch.
    pub fn honesty_beam_reveal_ok(&self) -> bool {
        self.beam_fields.iter().any(|f| f.reveal_applications > 0)
            && (PARTICLE_REVEAL_RANGE - 50.0).abs() < 0.01
    }

    /// Apply due TotalScorchMarks / GroundHitFX / RevealRange residual events.
    ///
    /// Retail (STATUS_FIRING): when `m_nextScorchMarkFrame <= now`, spawn scorch,
    /// run GroundHitFX, and doShroudReveal/undoShroudReveal at current target with
    /// RevealRange. Host residual records honesty counters + last scorch position
    /// (fail-closed vs full TheGameClient::addScorch GPU / partition shroud cells
    /// without a wired ShroudManager hook from this registry).
    pub fn apply_due_beam_scorch_reveals(
        &mut self,
        current_frame: u32,
    ) -> Vec<HostParticleScorchRevealEvent> {
        let mut events = Vec::new();
        for field in &mut self.beam_fields {
            // Catch up all due scorch marks (may be multi if frames skipped).
            while field.is_due_scorch(current_frame) {
                let pulse_idx = particle_scorch_pulse_index(field.scorch_marks_made);
                let epicenter = field.residual_epicenter(pulse_idx);
                let scorch_r = particle_scorch_radius(field.spawn_frame, current_frame);
                field.scorch_marks_made = field.scorch_marks_made.saturating_add(1);
                field.ground_hit_fx_applications =
                    field.ground_hit_fx_applications.saturating_add(1);
                field.reveal_applications = field.reveal_applications.saturating_add(1);
                field.last_scorch_position = epicenter;
                field.last_scorch_radius = scorch_r;
                let scheduled =
                    particle_next_scorch_frame(field.spawn_frame, field.scorch_marks_made);
                // Advance by schedule factor; allow multi-mark catch-up when
                // frames were skipped (do not clamp to current+1 inside the loop).
                field.next_scorch_frame = scheduled.max(field.next_scorch_frame.saturating_add(1));
                events.push(HostParticleScorchRevealEvent {
                    field_id: field.id,
                    source_object: field.source_object,
                    source_team: field.source_team,
                    position: epicenter,
                    scorch_radius: scorch_r,
                    reveal_range: PARTICLE_REVEAL_RANGE,
                    scorch_mark_index: field.scorch_marks_made,
                });
            }
        }
        events.sort_by_key(|e| (e.field_id, e.scorch_mark_index));
        events
    }

    /// Combined host path honesty: a completed strike exists for `kind`.
    /// NuclearMissile also requires residual radiation field spawn.
    /// AnthraxBomb also requires residual toxin field spawn.
    /// SpectreGunship also requires residual orbit field spawn.
    /// ParticleCannon also requires residual continuous beam field spawn.
    pub fn honesty_host_path_ok(&self, kind: HostSuperweaponKind) -> bool {
        if !self.honesty_complete_ok(kind) {
            return false;
        }
        if kind == HostSuperweaponKind::NuclearMissile {
            return self.honesty_radiation_ok();
        }
        if kind == HostSuperweaponKind::AnthraxBomb {
            return self.honesty_toxin_ok();
        }
        if kind == HostSuperweaponKind::SpectreGunship {
            return self.honesty_orbit_ok();
        }
        if kind == HostSuperweaponKind::ParticleCannon {
            return self.honesty_beam_ok();
        }
        true
    }
}
