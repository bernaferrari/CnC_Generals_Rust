//! Immutable receiving Die declarations; runtime receipts live on the Object.
use crate::assets::ini_parser::BehaviorModuleDefinition;
use crate::game_logic::host_fx_list_die::{fx_list_die_from_behavior_attrs, HostFxListDieData};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct NativeDieRuntime {
    pub(crate) completed: std::collections::BTreeSet<u32>,
    pub(crate) ai_dead: bool,
    pub(crate) pending_create: Option<u32>,
    /// Persisted by the versioned NDIE continuation, alongside the legacy receipts.
    #[serde(skip)]
    pub(crate) slow_death: Option<NativeSlowDeath>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub(crate) struct NativeSlowDeath {
    pub(crate) source_index: u32,
    pub(crate) midpoint_frame: u32,
    pub(crate) flight: SlowDeathFlight,
    pub(crate) helicopter: Option<NativeHelicopterFlight>,
}

/// Mutable values retained by HelicopterSlowDeathBehavior::xfer.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub(crate) struct NativeHelicopterFlight {
    pub(crate) forward_angle: f32,
    pub(crate) forward_speed: f32,
    pub(crate) self_spin: f32,
    pub(crate) spin_towards_maximum: bool,
    pub(crate) last_spin_update: u32,
    pub(crate) blade_frames: u32,
    pub(crate) hit_ground: Option<u32>,
    pub(crate) retired: bool,
}


#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum SlowDeathFlight {
    #[default]
    NotFlung,
    Flailing,
    Snagged,
    Bouncing,
    BouncingWithParachute,
    LandedInTree,
}

impl SlowDeathFlight {
    pub(crate) fn is_airborne(self) -> bool {
        matches!(self, Self::Flailing | Self::Snagged)
    }

    pub(crate) fn apply_model(self, object: &mut super::Object) {
        use game_engine::common::bit_flags::ModelConditionFlags as M;
        let flailing = 1u128 << M::EXPLODED_FLAILING;
        let bouncing = 1u128 << M::EXPLODED_BOUNCING;
        let parachuting = 1u128 << M::PARACHUTING;
        let set = match self {
            Self::NotFlung => return,
            Self::Flailing => flailing,
            Self::Snagged | Self::LandedInTree => parachuting,
            Self::Bouncing => bouncing,
            Self::BouncingWithParachute => bouncing | parachuting,
        };
        object.model_condition_bits =
            (object.model_condition_bits & !(flailing | bouncing)) | set;
    }
}
#[derive(Debug, Clone)]
pub(crate) struct NativeDieDeclaration {
    pub(crate) source_index: u32,
    pub(crate) module_tag: Option<String>,
    pub(crate) authored_identity: Vec<(String, String)>,
    pub(crate) gate: NativeDieGate,
    pub(crate) kind: NativeDieKind,
}
#[derive(Debug, Clone)]
pub(crate) struct NativeDieGate {
    death_types: u32,
    veterancy_levels: u32,
    exempt_status: u64,
    required_status: u64,
}
impl NativeDieGate {
    pub(crate) fn allows(
        &self,
        object: &super::Object,
        death: super::host_usa_pilot::HostDeathType,
    ) -> bool {
        HostFxListDieData {
            death_types: self.death_types,
            veterancy_levels: self.veterancy_levels,
            exempt_status: self.exempt_status,
            required_status: self.required_status,
            ..Default::default()
        }
        .leftover_die_mux_allows(
            death,
            object.experience.level,
            object.object_status_mask_for_queries(),
        )
    }
}
#[derive(Debug, Clone)]
pub(crate) struct NativeDieFx {
    name: Option<String>,
    audio: Option<String>,
    orient: bool,
    starts_active: bool,
    conflicts: Vec<String>,
    triggers: Vec<String>,
    all_triggers: bool,
}
impl NativeDieFx {
    pub(crate) fn configuration(&self) -> HostFxListDieData {
        HostFxListDieData {
            death_fx: self.name.clone(),
            death_audio: self.audio.clone(),
            orient_to_object: self.orient,
            starts_active: self.starts_active,
            upgrade_active: self.starts_active,
            conflicts_with: self.conflicts.clone(),
            triggered_by: self.triggers.clone(),
            requires_all_triggers: self.all_triggers,
            ..Default::default()
        }
    }
}
#[derive(Debug, Clone)]
pub(crate) struct NativeDieCreate {
    pub(crate) ocl_name: String,
    pub(crate) spawn_templates: Vec<String>,
    pub(crate) transfer_previous_health: bool,
}
impl NativeDieCreate {
    pub(crate) fn standalone_configuration(
        &self,
    ) -> super::host_create_object_die::HostCreateObjectDieData {
        super::host_create_object_die::HostCreateObjectDieData {
            ocl_name: self.ocl_name.clone(),
            spawn_templates: self.spawn_templates.clone(),
            transfer_previous_health: self.transfer_previous_health,
            fired: false,
        }
    }
}
fn bind_creation(list: &str, transfer: bool) -> Option<NativeDieCreate> {
    let config = super::host_create_object_die::declaration_from_creation_list(list, transfer)?;
    Some(NativeDieCreate {
        ocl_name: config.ocl_name,
        spawn_templates: config.spawn_templates,
        transfer_previous_health: config.transfer_previous_health,
    })
}
#[derive(Debug, Clone)]
pub(crate) enum NativeSlowFamily {
    Ordinary(std::sync::Arc<NativeSlowDeathBase>),
    Jet(std::sync::Arc<NativeJetSlowDeath>),
    Helicopter(std::sync::Arc<NativeHelicopterSlowDeath>),
}

impl NativeSlowFamily {
    pub(crate) fn base(&self) -> &std::sync::Arc<NativeSlowDeathBase> {
        match self {
            Self::Ordinary(base) => base,
            Self::Jet(jet) => &jet.base,
            Self::Helicopter(helicopter) => &helicopter.base,
        }
    }
}

#[derive(Debug)]
pub(crate) struct NativeJetSlowDeath {
    pub(crate) base: std::sync::Arc<NativeSlowDeathBase>,
    pub(crate) effects: super::host_jet_slow_death::HostJetSlowDeathFx,
    pub(crate) roll_rate: f32,
    pub(crate) roll_rate_delta: f32,
    pub(crate) pitch_rate: f32,
    pub(crate) fall_how_fast: f32,
}

#[derive(Debug)]
pub(crate) struct NativeHelicopterSlowDeath {
    pub(crate) base: std::sync::Arc<NativeSlowDeathBase>,
    pub(crate) effects: super::host_helicopter_slow_death::HostHelicopterSlowDeathFx,
    pub(crate) spiral_turn_rate: f32,
    pub(crate) forward_speed: f32,
    pub(crate) forward_damping: f32,
    pub(crate) min_spin: f32,
    pub(crate) max_spin: f32,
    pub(crate) spin_update_delay: f32,
    pub(crate) spin_update_amount: f32,
    pub(crate) fall_how_fast: f32,
    /// Host Locomotor stores deceleration in world units/sec².
    pub(crate) braking: f32,
    pub(crate) blade_delay: (f32, f32),
    pub(crate) blade_bone: Option<String>,
    pub(crate) particle_bone: Option<String>,
    pub(crate) particle_location: glam::Vec3,
    pub(crate) eject_pilot: Option<String>,
}

impl NativeHelicopterSlowDeath {
    fn bind(base: std::sync::Arc<NativeSlowDeathBase>, attrs: &[(&str, &str)]) -> Self {
        use game_engine::common::ini::INI;
        let get = |key: &str| {
            attrs.iter().find(|(name, _)| name.eq_ignore_ascii_case(key)).map(|(_, v)| *v)
        };
        let number = |key, parse: fn(&str) -> Result<f32, _>, default| {
            get(key).and_then(|value| parse(value).ok()).unwrap_or(default)
        };
        let name = |key| {
            get(key).map(str::trim).filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("None")).map(str::to_owned)
        };
        let duration = |key| number(key, INI::parse_duration_real, 0.0);
        let mut effects = super::host_helicopter_slow_death::heli_slow_death_fx_from_behavior_attrs(attrs);
        effects.delay_from_ground_frames = duration("DelayFromGroundToFinalDeath");
        let particle_location = get("AttachParticleLoc")
            .and_then(|value| INI::parse_coord_3d(&value.split_whitespace().collect::<Vec<_>>()).ok())
            .map_or(glam::Vec3::ZERO, |(x, y, z)| glam::Vec3::new(x, z, y));
        let fps = game_engine::common::game_common::LOGICFRAMES_PER_SECOND as f32;
        Self {
            base,
            effects,
            spiral_turn_rate: number("SpiralOrbitTurnRate", INI::parse_angular_velocity_real, 0.0),
            forward_speed: number("SpiralOrbitForwardSpeed", INI::parse_velocity_real, 0.0),
            forward_damping: number("SpiralOrbitForwardSpeedDamping", INI::parse_real, 1.0),
            min_spin: number("MinSelfSpin", INI::parse_angular_velocity_real, 0.0),
            max_spin: number("MaxSelfSpin", INI::parse_angular_velocity_real, 0.0),
            spin_update_delay: duration("SelfSpinUpdateDelay"),
            spin_update_amount: number("SelfSpinUpdateAmount", INI::parse_angle_real, 0.0) / fps,
            fall_how_fast: number("FallHowFast", INI::parse_percent_to_real, 0.0),
            braking: number("MaxBraking", INI::parse_real, 99_999.0 * fps * fps),
            blade_delay: (duration("MinBladeFlyOffDelay"), duration("MaxBladeFlyOffDelay")),
            blade_bone: name("BladeBoneName"),
            particle_bone: name("AttachParticleBone"),
            particle_location,
            eject_pilot: name("OCLEjectPilot"),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct NativeSlowDeathBase {
    pub(crate) phases: [SlowDeathEffects; 3],
    pub(crate) fling: Option<SlowDeathFling>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SlowDeathFling {
    pub(crate) min_force: f32,
    pub(crate) max_force: f32,
    pub(crate) min_pitch: f32,
    pub(crate) max_pitch: f32,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum SlowDeathEffectPhase {
    Initial = 0,
    Midpoint = 1,
    Final = 2,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct SlowDeathEffects {
    pub(crate) fx: Vec<String>,
    pub(crate) ocls: Vec<String>,
    pub(crate) weapons: Vec<String>,
}

fn parse_slow_death_effects(attrs: &[(&str, &str)]) -> [SlowDeathEffects; 3] {
    let mut phases: [SlowDeathEffects; 3] = Default::default();
    for (key, value) in attrs {
        if !["FX", "OCL", "Weapon"].iter().any(|field| key.eq_ignore_ascii_case(field)) {
            continue;
        }
        for line in value.lines() {
            let Some((phase, rest)) = line.trim().split_once(char::is_whitespace) else { continue };
            let phase = match phase.to_ascii_uppercase().as_str() {
                "INITIAL" => SlowDeathEffectPhase::Initial,
                "MIDPOINT" => SlowDeathEffectPhase::Midpoint,
                "FINAL" => SlowDeathEffectPhase::Final,
                _ => continue,
            };
            let effects = &mut phases[phase as usize];
            let list = if key.eq_ignore_ascii_case("FX") {
                &mut effects.fx
            } else if key.eq_ignore_ascii_case("OCL") {
                &mut effects.ocls
            } else if key.eq_ignore_ascii_case("Weapon") {
                &mut effects.weapons
            } else {
                continue;
            };
            list.extend(rest.split_whitespace().map(str::to_owned));
        }
    }
    phases
}
#[derive(Debug, Clone)]
pub(crate) enum NativeDieKind {
    Fx(NativeDieFx),
    Create {
        config: Option<NativeDieCreate>,
    },
    Crate {
        names: Box<[String]>,
    },
    Crush,
    TechBuilding,
    Keep,
    Destroy,
    Minefield,
    NeutronBlast(super::host_neutron_shell::NeutronBlastDefinition),
    Instant {
        config: super::host_instant_death::HostInstantDeathIni,
        creations: Vec<Option<NativeDieCreate>>,
    },
    Slow {
        family: NativeSlowFamily,
        config: super::host_slow_death::HostSlowDeathIni,
    },
    Topple,
    Collapse,
    DeadWeapon,
    Production,
    /// C++ SpecialPowerCompletionDieModuleData::m_specialPowerName
    /// (`SpecialPowerTemplate`).
    Completion { power: String },
    Dam,
    Pilot {
        ground_creation_list: Option<String>,
        air_creation_list: Option<String>,
    },
    Spawn,
    /// C++ UpgradeDieModuleData::m_upgradeName (`UpgradeToRemove`).
    Upgrade {
        upgrade_to_remove: String,
    },
    Rebuild,
    Contain {
        overlord: bool,
    },
    Bridge,
    Parking,
}
pub(crate) fn bind(modules: &[BehaviorModuleDefinition]) -> Vec<NativeDieDeclaration> {
    modules
        .iter()
        .enumerate()
        .filter_map(|(index, module)| {
            let attrs: Vec<_> = module
                .attributes
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect();
            let fx = fx_list_die_from_behavior_attrs(&attrs);
            let kind = match module.class_name.to_ascii_lowercase().as_str() {
                "fxlistdie" => NativeDieKind::Fx(NativeDieFx {
                    name: fx.death_fx.clone(),
                    audio: fx.death_audio.clone(),
                    orient: fx.orient_to_object,
                    starts_active: fx.starts_active,
                    conflicts: fx.conflicts_with.clone(),
                    triggers: fx.triggered_by.clone(),
                    all_triggers: fx.requires_all_triggers,
                }),
                "createobjectdie" => NativeDieKind::Create {
                    config: bind_creation(
                        module.attribute("CreationList").unwrap_or(""),
                        module
                            .attribute("TransferPreviousHealth")
                            .is_some_and(|value| value.eq_ignore_ascii_case("yes")),
                    ),
                },
                "createcratedie" => NativeDieKind::Crate {
                    names: Box::default(),
                },
                "crushdie" => NativeDieKind::Crush,
                "techbuildingbehavior" => NativeDieKind::TechBuilding,
                "keepobjectdie" => NativeDieKind::Keep,
                "destroydie" => NativeDieKind::Destroy,
                "minefieldbehavior" => NativeDieKind::Minefield,
                "neutronblastbehavior" => {
                    use super::host_neutron_shell::{
                        NEUTRON_AFFECT_AIRBORNE_DEFAULT, NEUTRON_AFFECT_ALLIES,
                        NEUTRON_BLAST_DEFAULT_RADIUS, NeutronBlastDefinition,
                    };
                    use game_engine::common::ini::INI;
                    let radius = module
                        .attribute("BlastRadius")
                        .map_or(Ok(NEUTRON_BLAST_DEFAULT_RADIUS), INI::parse_real)
                        .ok()?;
                    let affect_airborne = module
                        .attribute("AffectAirborne")
                        .map_or(Ok(NEUTRON_AFFECT_AIRBORNE_DEFAULT), INI::parse_bool)
                        .ok()?;
                    let affect_allies = module
                        .attribute("AffectAllies")
                        .map_or(Ok(NEUTRON_AFFECT_ALLIES), INI::parse_bool)
                        .ok()?;
                    NativeDieKind::NeutronBlast(NeutronBlastDefinition {
                        radius,
                        affect_airborne,
                        affect_allies,
                    })
                }
                "instantdeathbehavior" => {
                    let config =
                        super::host_instant_death::instant_death_ini_from_behavior_attrs(&attrs);
                    let creations = config
                        .ocls
                        .iter()
                        .map(|list| bind_creation(list, false))
                        .collect();
                    NativeDieKind::Instant { config, creations }
                }
                "slowdeathbehavior" | "jetslowdeathbehavior" | "helicopterslowdeathbehavior" => {
                    let config = super::host_slow_death::slow_death_ini_from_behavior_attrs(&attrs);
                    use game_engine::common::ini::INI;
                    let base = || {
                        std::sync::Arc::new(NativeSlowDeathBase {
                            phases: parse_slow_death_effects(&attrs),
                            fling: (config.fling_force > 0.0).then(|| {
                                let pitch = config.fling_pitch_deg.to_radians();
                                let pitch_variance = module
                                    .attribute("FlingPitchVariance")
                                    .and_then(|value| INI::parse_angle_real(value).ok())
                                    .unwrap_or(0.0);
                                SlowDeathFling {
                                    min_force: config.fling_force,
                                    max_force: config.fling_force + config.fling_force_variance,
                                    min_pitch: pitch,
                                    max_pitch: pitch + pitch_variance,
                                }
                            }),
                        })
                    };
                    let family = if module.class_name.eq_ignore_ascii_case("JetSlowDeathBehavior") {
                        use super::host_jet_slow_death::jet_slow_death_fx_from_behavior_attrs;
                        let real = |key| {
                            module
                                .attribute(key)
                                .and_then(|value| INI::parse_real(value).ok())
                                .unwrap_or(0.0)
                        };
                        let percent = |key, default| {
                            module
                                .attribute(key)
                                .and_then(|value| INI::parse_percent_to_real(value).ok())
                                .unwrap_or(default)
                        };
                        NativeSlowFamily::Jet(std::sync::Arc::new(NativeJetSlowDeath {
                            base: base(),
                            effects: jet_slow_death_fx_from_behavior_attrs(&attrs),
                            roll_rate: real("RollRate"),
                            roll_rate_delta: percent("RollRateDelta", 1.0),
                            pitch_rate: real("PitchRate"),
                            fall_how_fast: percent("FallHowFast", 0.0),
                        }))
                    } else if module.class_name.eq_ignore_ascii_case("HelicopterSlowDeathBehavior") {
                        NativeSlowFamily::Helicopter(std::sync::Arc::new(
                            NativeHelicopterSlowDeath::bind(base(), &attrs),
                        ))
                    } else {
                        NativeSlowFamily::Ordinary(base())
                    };
                    NativeDieKind::Slow { family, config }
                }
                "structuretoppleupdate" => NativeDieKind::Topple,
                "structurecollapseupdate" => NativeDieKind::Collapse,
                "fireweaponwhendeadbehavior" => NativeDieKind::DeadWeapon,
                "productionupdate" => NativeDieKind::Production,
                "parkingplacebehavior" => NativeDieKind::Parking,
                "specialpowercompletiondie" => NativeDieKind::Completion {
                    power: module
                        .attribute("SpecialPowerTemplate")
                        .and_then(|value| value.split_whitespace().next())
                        .unwrap_or_default()
                        .to_owned(),
                },
                "damdie" => NativeDieKind::Dam,
                "ejectpilotdie" => {
                    let ocl = |field: &str| {
                        module.attribute(field)
                            .and_then(|value| value.split(';').next())
                            .and_then(|value| value.split_whitespace().next())
                            .filter(|name| !name.eq_ignore_ascii_case("None"))
                            .map(str::to_owned)
                    };
                    NativeDieKind::Pilot {
                        ground_creation_list: ocl("GroundCreationList"),
                        air_creation_list: ocl("AirCreationList"),
                    }
                }
                "spawnbehavior" => NativeDieKind::Spawn,
                "upgradedie" => NativeDieKind::Upgrade {
                    upgrade_to_remove: module
                        .attribute("UpgradeToRemove")
                        .unwrap_or_default()
                        .to_string(),
                },
                "rebuildholeexposedie" => NativeDieKind::Rebuild,
                // ModuleFactory.cpp317-330: registered OpenContain-derived
                // classes inherit its Die interface. OpenTopContain is the
                // existing host alias; no name-substring inference is needed.
                "cavecontain"
                | "opencontain"
                | "healcontain"
                | "garrisoncontain"
                | "internethackcontain"
                | "transportcontain"
                | "riderchangecontain"
                | "railedtransportcontain"
                | "mobnexuscontain"
                | "tunnelcontain"
                | "helixcontain"
                | "parachutecontain"
                | "opentopcontain" => NativeDieKind::Contain { overlord: false },
                "overlordcontain" => NativeDieKind::Contain { overlord: true },
                "bridgebehavior" | "bridgetowerbehavior" => NativeDieKind::Bridge,
                _ => return None,
            };
            Some(NativeDieDeclaration {
                source_index: index as u32,
                module_tag: module.module_tag.clone(),
                authored_identity: {
                    let mut fields: Vec<_> = module
                        .attributes
                        .iter()
                        .map(|(key, value)| (key.to_ascii_lowercase(), value.clone()))
                        .collect();
                    fields.push(("$class".into(), module.class_name.to_ascii_lowercase()));
                    fields.sort();
                    fields
                },
                gate: NativeDieGate {
                    death_types: fx.death_types,
                    veterancy_levels: fx.veterancy_levels,
                    exempt_status: fx.exempt_status,
                    required_status: fx.required_status,
                },
                kind,
            })
        })
        .collect()
}

impl NativeSlowDeath {
    pub(crate) fn family_in(self, declarations: &[NativeDieDeclaration]) -> Option<&NativeSlowFamily> {
        let declaration = declarations.iter().find(|slot| slot.source_index == self.source_index)?;
        match &declaration.kind {
            NativeDieKind::Slow { family, .. } => Some(family),
            _ => None,
        }
    }
}

impl NativeDieRuntime {
    pub(crate) fn validate(&self, declarations: &[NativeDieDeclaration], has_ai: bool) -> bool {
        (!self.ai_dead || has_ai)
            && self.slow_death.is_none_or(|slow| {
                slow.family_in(declarations)
                    .is_some_and(|family| {
                        (slow.flight == SlowDeathFlight::NotFlung || family.base().fling.is_some())
                            && matches!(family, NativeSlowFamily::Helicopter(_)) == slow.helicopter.is_some()
                    })
            })
            && self.completed.iter().all(|index| {
                declarations
                    .iter()
                    .any(|declaration| declaration.source_index == *index)
            })
            && self.pending_create.is_none_or(|index| {
                !self.completed.contains(&index)
                    && declarations.iter().any(|declaration| {
                        declaration.source_index == index
                            && matches!(declaration.kind, NativeDieKind::Create { .. })
                    })
            })
    }
}

impl NativeDieDeclaration {
    /// Bind ordered CrateData rows from the template's admitted module data.
    pub(crate) fn bind_crate_names(
        &mut self,
        modules: &game_engine::common::thing::thing_template::ModuleInfo,
    ) {
        use gamelogic::object::die::CreateCrateDieModuleData;

        let NativeDieKind::Crate { names } = &mut self.kind else {
            return;
        };
        *names = modules
            .iter()
            .find(|entry| {
                entry.name.as_str().eq_ignore_ascii_case("CreateCrateDie")
                    && Some(entry.module_tag.as_str()) == self.module_tag.as_deref()
            })
            .and_then(|entry| {
                let data = entry.data.read().into_downcast::<CreateCrateDieModuleData>()?;
                Some(data.crate_name_list.clone().into_boxed_slice())
            })
            .unwrap_or_default();
    }

    fn bound_creation_identity(&self) -> Option<Vec<Option<(&str, &[String], bool)>>> {
        fn identity(config: &NativeDieCreate) -> (&str, &[String], bool) {
            (
                config.ocl_name.as_str(),
                config.spawn_templates.as_slice(),
                config.transfer_previous_health,
            )
        }
        match &self.kind {
            NativeDieKind::Create { config } => Some(vec![config.as_ref().map(identity)]),
            NativeDieKind::Instant { creations, .. } => Some(
                creations
                    .iter()
                    .map(|config| config.as_ref().map(identity))
                    .collect(),
            ),
            _ => None,
        }
    }

    pub(crate) fn persistence_identity(&self) -> String {
        // Versioned, deterministic identity of effective immutable authoring;
        // runtime receipts and HashMap iteration order never enter this value.
        serde_json::to_string(&(
            1u32,
            self.source_index,
            &self.module_tag,
            &self.authored_identity,
            self.bound_creation_identity(),
        ))
        .expect("Die declaration identity contains only strings and integers")
    }
}
