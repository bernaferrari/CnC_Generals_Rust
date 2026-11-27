//! Receiving effective Weapon fire references and immutable OCL content.
//! CPP Weapon::fireWeaponTemplate889–949 selects exact source veterancy rows;
//! OCL definitions admit once and remain content across session resets.
use crate::game_logic::{GameLogic, VeterancyLevel};
use generals_sim::object_creation_list::{
    ObjectCreationList, ObjectCreationListDefinitions, ObjectCreationLists,
};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::game_logic) struct ResolvedWeaponFireEffects {
    pub fire_fx: String,
    pub fire_ocl: String,
    pub play_fx_when_stealthed: bool,
}
#[derive(Debug, Default)]
pub(in crate::game_logic) struct WeaponEffectDefinitions {
    fire: HashMap<String, [ResolvedWeaponFireEffects; 4]>,
    ocls: ObjectCreationLists,
}
impl WeaponEffectDefinitions {
    pub(in crate::game_logic) fn has_ocl(&self, name: &str) -> bool {
        self.ocls.get(name).is_some()
    }

    pub(in crate::game_logic) fn fire(
        &self,
        name: &str,
        rank: VeterancyLevel,
    ) -> Option<ResolvedWeaponFireEffects> {
        let index = match rank {
            VeterancyLevel::Rookie => 0,
            VeterancyLevel::Veteran => 1,
            VeterancyLevel::Elite => 2,
            VeterancyLevel::Heroic => 3,
        };
        self.fire.get(name.trim()).map(|rows| rows[index].clone())
    }
    pub(in crate::game_logic) fn ocl(&self, name: &str) -> Option<ObjectCreationList> {
        self.ocls.get(name).cloned()
    }
}
impl GameLogic {
    /// Exact receiving named definition; absence never selects a global store.
    pub(crate) fn object_creation_list_definition(&self, name: &str) -> Option<ObjectCreationList> {
        self.weapon_effect_definitions.ocl(name)
    }

    pub(in crate::game_logic) fn has_object_creation_list(&self, name: &str) -> bool {
        self.weapon_effect_definitions.has_ocl(name)
    }

    /// C++ OCLSpecialPower::findOCL — first owned UpgradeOCL else the OCL
    /// token on the module matching `power_template`, resolved against this
    /// session's receiving store only.
    pub(in crate::game_logic) fn receiving_special_power_ocl(
        &self,
        source: &super::super::Object,
        power_template: &str,
    ) -> Option<ObjectCreationList> {
        let module = source
            .thing()
            .template
            .receiving_object_definition
            .as_ref()?
            .behavior_modules
            .iter()
            .find(|module| {
                module.class_name.eq_ignore_ascii_case("OCLSpecialPower")
                    && module
                        .attribute("SpecialPowerTemplate")
                        .and_then(|value| value.split_whitespace().next())
                        .is_some_and(|name| name.eq_ignore_ascii_case(power_template))
            })?;
        let player = self
            .player_owner_for_host_object(source)
            .and_then(|pid| self.players.get(&pid));
        let override_ocl = module
            .attribute("UpgradeOCL")
            .into_iter()
            .flat_map(str::lines)
            .filter_map(|line| {
                let mut tokens = line.split_whitespace();
                Some((tokens.next()?, tokens.next()?))
            })
            .find_map(|(science, ocl)| {
                player
                    .is_some_and(|p| {
                        p.unlocked_sciences
                            .iter()
                            .any(|owned| owned.eq_ignore_ascii_case(science))
                    })
                    .then_some(ocl)
            });
        let name = override_ocl.or_else(|| {
            module
                .attribute("OCL")
                .and_then(|value| value.lines().next_back())
                .and_then(|line| line.split_whitespace().next())
        })?;
        self.object_creation_list_definition(name)
    }

    /// One immutable application definition set is captured before match workers.
    /// Map/Solo replaces named lists on a private effective map, never this base.
    pub(crate) fn admit_ocl_application_sources(
        &mut self,
        sources: &[String],
    ) -> Result<(), String> {
        self.weapon_effect_definitions.ocls.admit_application_sources(sources)
    }

    /// Admit the immutable OCL application catalog owned by this session.
    /// The application baseline is fixed before live Objects capture die
    /// declarations; later map-specific OCL changes use the override path.
    pub fn admit_object_creation_list_sources<I>(&mut self, sources: I) -> Result<(), String>
    where
        I: IntoIterator<Item = String>,
    {
        if !self.objects.registered_is_empty() {
            return Err("Object creation lists must be admitted before live Objects".into());
        }
        let sources: Vec<String> = sources.into_iter().collect();
        self.admit_ocl_application_sources(&sources)
    }
    pub(crate) fn ocl_application_definitions(
        &self,
    ) -> std::sync::Arc<ObjectCreationListDefinitions> {
        self.weapon_effect_definitions.ocls.application_definitions()
    }

    pub(crate) fn seed_ocl_application_definitions(
        &mut self,
        base: std::sync::Arc<ObjectCreationListDefinitions>,
    ) {
        self.weapon_effect_definitions.ocls.seed_application_definitions(base);
    }

    pub(crate) fn reset_ocl_map_definitions(&mut self) {
        self.weapon_effect_definitions.ocls.reset_to_application();
    }

    pub(crate) fn apply_receiving_ocl_overrides(
        &mut self,
        sources: &[String],
    ) -> Result<(), String> {
        self.weapon_effect_definitions.ocls.apply_map_overrides(sources)
    }

    pub(crate) fn admit_weapon_fire_definition(
        &mut self,
        name: &str,
        template: &generals_sim::weapon::WeaponDefinition,
    ) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("weapon fire definition name is blank".into());
        }
        if self.weapon_effect_definitions.fire.contains_key(name) {
            return Err(format!("weapon fire definition already admitted: {name}"));
        }
        let ranks = [
            gamelogic::common::VeterancyLevel::Regular,
            gamelogic::common::VeterancyLevel::Veteran,
            gamelogic::common::VeterancyLevel::Elite,
            gamelogic::common::VeterancyLevel::Heroic,
        ];
        let rows = std::array::from_fn(|index| ResolvedWeaponFireEffects {
            play_fx_when_stealthed: template.play_fx_when_stealthed,
            fire_fx: template
                .get_fire_fx(ranks[index])
                .map(|fx| fx.name().trim().to_owned())
                .unwrap_or_default(),
            // get_fire_ocl() performs ambient deferred resolution. Only the
            // retained immutable name is read here; the receiver admits OCL
            // content independently and never publishes a global store.
            fire_ocl: template
                .get_fire_ocl_name(ranks[index])
                .map(|name| name.trim().to_owned())
                .unwrap_or_default(),
        });
        self.weapon_effect_definitions
            .fire
            .insert(name.to_owned(), rows);
        Ok(())
    }
    pub(crate) fn admit_weapon_ocl_definition(
        &mut self,
        name: &str,
        ocl: ObjectCreationList,
    ) -> Result<(), String> {
        self.weapon_effect_definitions.ocls.admit_definition(name, ocl)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{Team, ThingTemplate};
    fn native(prefix: &str) -> generals_sim::weapon::WeaponDefinition {
        let mut template = generals_sim::weapon::WeaponDefinition::new("SameWeapon".to_owned());
        for index in 0..4 {
            template.fire_fx[index] = Some(generals_sim::weapon::WeaponFxName::new(&format!(
                "{prefix}FX{index}"
            )));
            template.fire_ocl_names[index] = Some(format!("{prefix}OCL{index}"));
        }
        template
    }
    #[test]
    fn same_id_receiving_effect_definitions_survive_peer_reset_without_override() {
        let mut owner = GameLogic::new();
        let mut peer = GameLogic::new();
        for world in [&mut owner, &mut peer] {
            world
                .templates
                .insert("SameSource".into(), ThingTemplate::new("SameSource"));
        }
        let a = owner
            .create_object("SameSource", Team::USA, glam::Vec3::ZERO)
            .unwrap();
        let b = peer
            .create_object("SameSource", Team::USA, glam::Vec3::ZERO)
            .unwrap();
        assert_eq!(a, b);
        let mut admitted = native("Owner");
        owner
            .admit_weapon_fire_definition("SameWeapon", &admitted)
            .unwrap();
        peer.admit_weapon_fire_definition("SameWeapon", &native("Peer"))
            .unwrap();
        owner
            .admit_weapon_ocl_definition("OwnerOCL0", ObjectCreationList::default())
            .unwrap();
        admitted.fire_ocl_names[0] = Some("AfterAdmissionMutation".into());
        peer.reset();
        for (index, rank) in [
            VeterancyLevel::Rookie,
            VeterancyLevel::Veteran,
            VeterancyLevel::Elite,
            VeterancyLevel::Heroic,
        ]
        .into_iter()
        .enumerate()
        {
            let effects = owner
                .weapon_effect_definitions
                .fire("SameWeapon", rank)
                .unwrap();
            assert_eq!(effects.fire_fx, format!("OwnerFX{index}"));
            assert_eq!(effects.fire_ocl, format!("OwnerOCL{index}"));
            assert_eq!(
                peer.weapon_effect_definitions
                    .fire("SameWeapon", rank)
                    .unwrap()
                    .fire_fx,
                format!("PeerFX{index}")
            );
        }
        owner.reset();
        assert!(owner.weapon_effect_definitions.ocl("OwnerOCL0").is_some());
        assert!(peer.weapon_effect_definitions.ocl("OwnerOCL0").is_none());
        assert!(
            owner
                .admit_weapon_fire_definition("SameWeapon", &native("Replacement"))
                .is_err()
        );
        assert!(
            owner
                .admit_weapon_ocl_definition("OwnerOCL0", ObjectCreationList::default())
                .is_err()
        );
        assert!(
            owner
                .admit_weapon_fire_definition(" ", &native("Invalid"))
                .is_err()
        );
        assert!(
            owner
                .admit_weapon_ocl_definition(" ", ObjectCreationList::default())
                .is_err()
        );
        assert_eq!(
            owner
                .weapon_effect_definitions
                .fire("SameWeapon", VeterancyLevel::Rookie)
                .unwrap()
                .fire_fx,
            "OwnerFX0"
        );
    }
}

#[cfg(test)]
#[path = "weapon_effect_definitions/ocl_content_tests.rs"]
mod ocl_content_tests;
