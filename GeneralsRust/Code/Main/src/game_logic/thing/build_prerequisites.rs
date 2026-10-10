//! Names resolved once at definition admission; gameplay borrows the session.
use super::*;
use game_engine::common::rts::{ProductionPrerequisite, ScienceType, ThingTemplateHandle};

#[derive(Debug, Clone, Default)]
pub(crate) struct PrerequisiteDefinitions {
    templates: HashMap<ThingTemplateHandle, String>,
    sciences: HashMap<ScienceType, String>,
}

impl PrerequisiteDefinitions {
    pub(super) fn admit(prereqs: &[ProductionPrerequisite]) -> Self {
        Self::resolve(
            prereqs,
            |handle| {
                let id = u16::try_from(handle.value()).ok()?;
                let guard = game_engine::common::thing::thing_factory::try_get_thing_factory()?;
                Some(
                    guard
                        .as_ref()?
                        .find_by_template_id(id)?
                        .get_name()
                        .to_string(),
                )
            },
            |science| {
                let store = game_engine::common::rts::get_science_store()?;
                let name = store.get_internal_name_for_science(science);
                (!name.is_empty()).then(|| name.to_string())
            },
        )
    }

    pub(crate) fn resolve(
        prereqs: &[ProductionPrerequisite],
        template_name: impl Fn(ThingTemplateHandle) -> Option<String>,
        science_name: impl Fn(ScienceType) -> Option<String>,
    ) -> Self {
        let mut definitions = Self::default();
        for prereq in prereqs {
            for unit in prereq.get_unit_prereqs() {
                if let Some(handle) = unit.unit.filter(|handle| handle.is_valid()) {
                    if let Some(name) = (!unit.name.is_empty())
                        .then(|| unit.name.clone())
                        .or_else(|| template_name(handle))
                    {
                        definitions.templates.insert(handle, name);
                    }
                }
            }
            for &science in prereq.get_science_prereqs() {
                if let Some(name) = science_name(science) {
                    definitions.sciences.insert(science, name);
                }
            }
        }
        definitions
    }
}

impl ThingTemplate {
    /// CPP ThingTemplate.cpp:1454–1492; catalog lookup has already selected
    /// the final override. Reskin and build-variation identities remain data.
    pub(crate) fn is_equivalent_to(&self, other: &Self) -> bool {
        self.name.eq_ignore_ascii_case(&other.name)
            || self
                .reskinned_from
                .as_deref()
                .is_some_and(|base| base == other.name)
            || other
                .reskinned_from
                .as_deref()
                .is_some_and(|base| base == self.name)
            || matches!((&self.reskinned_from, &other.reskinned_from), (Some(a), Some(b)) if a == b)
            || self
                .build_variations
                .iter()
                .any(|name| name.eq_ignore_ascii_case(&other.name))
            || other
                .build_variations
                .iter()
                .any(|name| name.eq_ignore_ascii_case(&self.name))
    }

    pub(crate) fn prerequisites_satisfied(&self, world: &GameLogic, player: &Player) -> bool {
        self.production_prerequisites.iter().all(|prereq| {
            // Retain Common's original science/AND/OR/MAX_PREREQ algorithm.
            prereq.is_satisfied_with_counter(
                |science| {
                    self.prerequisite_definitions
                        .sciences
                        .get(&science)
                        .is_some_and(|name| {
                            player.unlocked_sciences.iter().any(|owned| {
                                owned.eq_ignore_ascii_case(name)
                                    || owned.eq_ignore_ascii_case(&format!("SCIENCE_{name}"))
                                    || name.eq_ignore_ascii_case(&format!("SCIENCE_{owned}"))
                            })
                        })
                },
                |handles, ignore_dead, counts| {
                    counts.fill(0);
                    // CPP Team::countObjectsByThingTemplate counts an object
                    // only for its first matching entry, even for duplicate
                    // or equivalent prerequisite templates.
                    for object in world.host_objects().values() {
                        if object.owner_player_id != Some(player.id)
                            || object.status.under_construction
                            || (ignore_dead
                                && (!object.is_alive() || object.status.effectively_dead))
                        {
                            continue;
                        }
                        let actual = world
                            .templates
                            .get(&object.template_name)
                            .unwrap_or(&object.thing().template);
                        for (index, (&handle, count)) in
                            handles.iter().zip(counts.iter_mut()).enumerate()
                        {
                            let required = prereq
                                .get_unit_prereqs()
                                .get(index)
                                .filter(|unit| !unit.name.is_empty())
                                .map(|unit| &unit.name)
                                .or_else(|| self.prerequisite_definitions.templates.get(&handle));
                            if required
                                .and_then(|name| world.templates.get(name))
                                .is_some_and(|wanted| actual.is_equivalent_to(wanted))
                            {
                                *count += 1;
                                break;
                            }
                        }
                    }
                },
            )
        })
    }
}

impl GameLogic {
    /// CPP Player::canBuild: permission, status, prerequisites, then type cap.
    /// Cash, faction similarity and producer availability are separate queries.
    pub(in crate::game_logic) fn script_player_can_build_template(
        &self,
        player: &Player,
        name: &str,
    ) -> bool {
        use crate::game_logic::host_production_buildable_command_residual::{
            BSTATUS_IGNORE_PREREQUISITES, BSTATUS_NO, BSTATUS_ONLY_BY_AI,
        };
        let Some(template) = self.templates.get(name) else {
            return false;
        };
        if !player.allowed_to_build(template.is_kind_of(KindOf::Structure)) {
            return false;
        }
        match template.buildable_status {
            BSTATUS_NO => return false,
            BSTATUS_IGNORE_PREREQUISITES => return true,
            BSTATUS_ONLY_BY_AI if player.is_human => return false,
            _ => {}
        }
        template.prerequisites_satisfied(self, player)
            && self.can_build_more_of_type(Some(player.id), player.team, name)
    }
}
