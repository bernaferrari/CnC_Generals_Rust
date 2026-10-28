//! Immutable receiving Die declarations; runtime receipts live on the Object.
use crate::assets::ini_parser::BehaviorModuleDefinition;
use crate::game_logic::host_fx_list_die::{fx_list_die_from_behavior_attrs, HostFxListDieData};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct NativeDieRuntime {
    pub(crate) completed: std::collections::BTreeSet<u32>,
    pub(crate) ai_dead: bool,
    pub(crate) pending_create: Option<u32>,
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
            super::host_fx_list_die::leftover_veterancy_from_host(object.experience.level),
            object.object_status_bits,
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
    Ordinary,
    Jet(super::host_jet_slow_death::HostJetSlowDeathFx),
    Helicopter(super::host_helicopter_slow_death::HostHelicopterSlowDeathFx),
}
#[derive(Debug, Clone)]
pub(crate) enum NativeDieKind {
    Fx(NativeDieFx),
    Create {
        config: Option<NativeDieCreate>,
    },
    Crush,
    Keep,
    Destroy,
    Minefield,
    Instant {
        config: super::host_instant_death::HostInstantDeathIni,
        creations: Vec<Option<NativeDieCreate>>,
    },
    Slow {
        family: NativeSlowFamily,
        config: super::host_slow_death::HostSlowDeathIni,
        probability: i32,
    },
    Topple,
    Collapse,
    DeadWeapon,
    Production,
    Completion,
    Dam,
    Pilot,
    Spawn,
    Upgrade,
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
                "crushdie" => NativeDieKind::Crush,
                "keepobjectdie" => NativeDieKind::Keep,
                "destroydie" => NativeDieKind::Destroy,
                "minefieldbehavior" => NativeDieKind::Minefield,
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
                    NativeDieKind::Slow {
                        family: match module.class_name.to_ascii_lowercase().as_str() {
                            "jetslowdeathbehavior" => NativeSlowFamily::Jet(super::host_jet_slow_death::jet_slow_death_fx_from_behavior_attrs(&attrs)),
                            "helicopterslowdeathbehavior" => NativeSlowFamily::Helicopter(super::host_helicopter_slow_death::heli_slow_death_fx_from_behavior_attrs(&attrs)),
                            _ => NativeSlowFamily::Ordinary,
                        },
                        config: super::host_slow_death::slow_death_ini_from_behavior_attrs(&attrs),
                        probability: module
                            .attribute("ProbabilityModifier")
                            .and_then(|v| v.parse().ok())
                            .unwrap_or(10),
                    }
                }
                "structuretoppleupdate" => NativeDieKind::Topple,
                "structurecollapseupdate" => NativeDieKind::Collapse,
                "fireweaponwhendeadbehavior" => NativeDieKind::DeadWeapon,
                "productionupdate" => NativeDieKind::Production,
                "parkingplacebehavior" => NativeDieKind::Parking,
                "specialpowercompletiondie" => NativeDieKind::Completion,
                "damdie" => NativeDieKind::Dam,
                "ejectpilotdie" => NativeDieKind::Pilot,
                "spawnbehavior" => NativeDieKind::Spawn,
                "upgradedie" => NativeDieKind::Upgrade,
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

impl NativeDieRuntime {
    pub(crate) fn validate(&self, declarations: &[NativeDieDeclaration], has_ai: bool) -> bool {
        (!self.ai_dead || has_ai)
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
