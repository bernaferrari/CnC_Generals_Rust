//! Object::updateUpgradeModules / removeUpgrade and the typed UpgradeMux pass.
use super::*;
use gamelogic::object::upgrade::{
    ArmorUpgradeModuleData, CommandSetUpgradeModuleData, ModelConditionUpgradeModuleData,
    ReplaceObjectUpgradeModuleData, SubObjectsUpgradeModuleData,
};

struct AuthoredObjectCreationUpgrade {
    state_index: usize,
    data: ModuleDataView<
        gamelogic::upgrade::modules::object_creation::ObjectCreationUpgradeModuleData,
    >,
}

struct AuthoredStatusBitsUpgrade {
    state_index: usize,
    data: ModuleDataView<
        gamelogic::object::upgrade::status_bits_upgrade::StatusBitsUpgradeModuleData,
    >,
}

struct AuthoredPassengersFireUpgrade {
    source_index: usize,
    data: ModuleDataView<
        gamelogic::object::upgrade::passengers_fire_upgrade::PassengersFireUpgradeModuleData,
    >,
}

impl AuthoredPassengersFireUpgrade {
    fn executed_key(&self) -> String {
        format!("PassengersFireUpgrade#{}", self.source_index)
    }
}

struct AuthoredMaxHealthUpgrade {
    source_index: usize,
    data:
        ModuleDataView<gamelogic::object::upgrade::max_health_upgrade::MaxHealthUpgradeModuleData>,
}

impl AuthoredMaxHealthUpgrade {
    fn executed_key(&self) -> String {
        format!("MaxHealthUpgrade#{}", self.source_index)
    }
}

#[derive(Clone, Copy)]
enum AuthoredWeaponUpgradeKind {
    Set,
    Bonus,
}

struct AuthoredWeaponUpgrade {
    source_index: usize,
    kind: AuthoredWeaponUpgradeKind,
    mux: ModuleDataView<gamelogic::upgrade::modules::upgrade_mux::UpgradeMuxData>,
}

impl AuthoredWeaponUpgrade {
    fn executed_key(&self) -> String {
        let class = match self.kind {
            AuthoredWeaponUpgradeKind::Set => "WeaponSetUpgrade",
            AuthoredWeaponUpgradeKind::Bonus => "WeaponBonusUpgrade",
        };
        format!("{class}#{}", self.source_index)
    }
}

struct AuthoredReplaceObjectUpgrade {
    source_index: usize,
    data: ModuleDataView<ReplaceObjectUpgradeModuleData>,
}

impl AuthoredReplaceObjectUpgrade {
    fn executed_key(&self) -> String {
        format!("ReplaceObjectUpgrade#{}", self.source_index)
    }
}

struct AuthoredArmorUpgrade {
    source_index: usize,
    data: ModuleDataView<ArmorUpgradeModuleData>,
}

impl AuthoredArmorUpgrade {
    fn executed_key(&self) -> String {
        format!("ArmorUpgrade#{}", self.source_index)
    }
}

struct AuthoredCommandSetUpgrade {
    source_index: usize,
    data: ModuleDataView<CommandSetUpgradeModuleData>,
}

impl AuthoredCommandSetUpgrade {
    fn executed_key(&self) -> String {
        format!("CommandSetUpgrade#{}", self.source_index)
    }
}

struct AuthoredPowerPlantUpgrade {
    executed_key: String,
    data: ModuleDataView<gamelogic::object::upgrade::PowerPlantUpgradeModuleData>,
}

struct AuthoredRadarUpgrade {
    state_index: usize,
    data: ModuleDataView<gamelogic::object::upgrade::RadarUpgradeModuleData>,
}

enum VisualUpgradeEffect {
    ModelCondition(ModuleDataView<ModelConditionUpgradeModuleData>),
    SubObjects(ModuleDataView<SubObjectsUpgradeModuleData>),
}

struct AuthoredVisualUpgrade {
    source_index: usize,
    effect: VisualUpgradeEffect,
}

impl AuthoredVisualUpgrade {
    fn executed_key(&self) -> String {
        let class = match self.effect {
            VisualUpgradeEffect::ModelCondition(_) => "ModelConditionUpgrade",
            VisualUpgradeEffect::SubObjects(_) => "SubObjectsUpgrade",
        };
        format!("{class}#{}", self.source_index)
    }
}

enum AuthoredOrderedUpgrade {
    ObjectCreation(AuthoredObjectCreationUpgrade),
    ReplaceObject(AuthoredReplaceObjectUpgrade),
    Armor(AuthoredArmorUpgrade),
    Visual(AuthoredVisualUpgrade),
    CommandSet(AuthoredCommandSetUpgrade),
    StatusBits(AuthoredStatusBitsUpgrade),
    CostModifier(AuthoredCostModifierUpgrade),
    PowerPlant(AuthoredPowerPlantUpgrade),
    Radar(AuthoredRadarUpgrade),
    PassengersFire(AuthoredPassengersFireUpgrade),
    MaxHealth(AuthoredMaxHealthUpgrade),
    Weapon(AuthoredWeaponUpgrade),
}

/// A single Object's scheduled callbacks, before any sibling can mutate its mask.
pub(super) struct AuthoredUpgradePass<'a> {
    object_id: ObjectId,
    birth: std::sync::Weak<()>,
    pending_upgrade: Option<&'a str>,
    modules: Vec<AuthoredOrderedUpgrade>,
    mask: gamelogic::upgrade::UpgradeMask,
}

impl AuthoredOrderedUpgrade {
    fn with_mux<R>(
        &self,
        f: impl FnOnce(&gamelogic::upgrade::modules::upgrade_mux::UpgradeMuxData) -> R,
    ) -> R {
        match self {
            Self::ObjectCreation(module) => f(&module.data.read().upgrade_mux_data),
            Self::ReplaceObject(module) => f(&module.data.read().upgrade_mux_data),
            Self::Armor(module) => f(&module.data.read().upgrade_mux_data),
            Self::Visual(module) => match &module.effect {
                VisualUpgradeEffect::ModelCondition(data) => f(&data.read().upgrade_mux_data),
                VisualUpgradeEffect::SubObjects(data) => f(&data.read().upgrade_mux_data),
            },
            Self::CommandSet(module) => f(&module.data.read().upgrade_mux_data),
            Self::StatusBits(module) => f(&module.data.read().upgrade_mux_data),
            Self::CostModifier(module) => f(&module.data.read().upgrade_mux_data),
            Self::PowerPlant(module) => f(&module.data.read().upgrade_mux_data),
            Self::Radar(module) => f(&module.data.read().upgrade_mux_data),
            Self::PassengersFire(module) => f(&module.data.read().upgrade_mux_data),
            Self::MaxHealth(module) => f(&module.data.read().upgrade_mux_data),
            Self::Weapon(module) => f(&module.mux.read()),
        }
    }
}

impl GameLogic {
    /// Collect this template's authored UpgradeMux modules and align the object latches.
    fn authored_ordered_upgrade_modules(
        &mut self,
        object_id: ObjectId,
    ) -> Vec<AuthoredOrderedUpgrade> {
        use game_engine::common::thing::module::ModuleData;
        let Some(object) = self.objects.get(&object_id) else {
            return Vec::new();
        };
        let Some(content) = self.receiving_module_content.as_ref() else {
            return Vec::new();
        };
        let Some(template) = content.admitted_template(&object.template_name) else {
            return Vec::new();
        };
        let mut object_creation_index = 0;
        let mut status_bits_index = 0;
        let mut cost_modifier_index = 0;
        let mut power_plant_index = 0;
        let mut radar_index = 0;
        // Filtering the admitted roster preserves its declaration order.
        let modules: Vec<_> = template
            .behavior_modules()
            .iter()
            .enumerate()
            .filter_map(|(source_index, module)| {
                use game_engine::common::thing::module::ModuleData;
                if module.name.as_str().eq_ignore_ascii_case("ObjectCreationUpgrade") {
                    let data = upgrade_data_view::<
                        gamelogic::upgrade::modules::object_creation::ObjectCreationUpgradeModuleData,
                    >(&module.data)?;
                    let state_index = object_creation_index;
                    object_creation_index += 1;
                    return Some(AuthoredOrderedUpgrade::ObjectCreation(
                        AuthoredObjectCreationUpgrade { state_index, data }
                    ));
                }
                if module.name.as_str().eq_ignore_ascii_case("ReplaceObjectUpgrade") {
                    let data = upgrade_data_view::<ReplaceObjectUpgradeModuleData>(&module.data)?;
                    return Some(AuthoredOrderedUpgrade::ReplaceObject(
                        AuthoredReplaceObjectUpgrade { source_index, data },
                    ));
                }
                if module.name.as_str().eq_ignore_ascii_case("ModelConditionUpgrade") {
                    let data = upgrade_data_view::<ModelConditionUpgradeModuleData>(&module.data)?;
                    return Some(AuthoredOrderedUpgrade::Visual(AuthoredVisualUpgrade {
                        source_index,
                        effect: VisualUpgradeEffect::ModelCondition(data),
                    }));
                }
                if module.name.as_str().eq_ignore_ascii_case("SubObjectsUpgrade") {
                    let data = upgrade_data_view::<SubObjectsUpgradeModuleData>(&module.data)?;
                    return Some(AuthoredOrderedUpgrade::Visual(AuthoredVisualUpgrade {
                        source_index,
                        effect: VisualUpgradeEffect::SubObjects(data),
                    }));
                }
                if module.name.as_str().eq_ignore_ascii_case("ArmorUpgrade") {
                    let data = upgrade_data_view::<ArmorUpgradeModuleData>(&module.data)?;
                    return Some(AuthoredOrderedUpgrade::Armor(AuthoredArmorUpgrade {
                        source_index, data,
                    }));
                }
                if module.name.as_str().eq_ignore_ascii_case("CommandSetUpgrade") {
                    let data = upgrade_data_view::<CommandSetUpgradeModuleData>(&module.data)?;
                    return Some(AuthoredOrderedUpgrade::CommandSet(
                        AuthoredCommandSetUpgrade { source_index, data },
                    ));
                }
                if module.name.as_str().eq_ignore_ascii_case("StatusBitsUpgrade") {
                    let data = upgrade_data_view::<
                        gamelogic::object::upgrade::status_bits_upgrade::StatusBitsUpgradeModuleData,
                    >(&module.data)?;
                    let state_index = status_bits_index;
                    status_bits_index += 1;
                    return Some(AuthoredOrderedUpgrade::StatusBits(
                        AuthoredStatusBitsUpgrade { state_index, data }
                    ));
                }
                if module.name.as_str().eq_ignore_ascii_case("CostModifierUpgrade") {
                    let data = upgrade_data_view::<
                        gamelogic::object::upgrade::cost_modifier_upgrade::CostModifierUpgradeModuleData,
                    >(&module.data)?;
                    let state_index = cost_modifier_index;
                    cost_modifier_index += 1;
                    return Some(AuthoredOrderedUpgrade::CostModifier(
                        AuthoredCostModifierUpgrade { state_index, data }
                    ));
                }
                if module.name.as_str().eq_ignore_ascii_case("PowerPlantUpgrade") {
                    let data = upgrade_data_view::<gamelogic::object::upgrade::PowerPlantUpgradeModuleData>(
                        &module.data,
                    )?;
                    let tag = module.module_tag.as_str();
                    let executed_key = super::super::authored_upgrade_modules::control_rods_upgrade_key(
                        power_plant_index,
                        (!tag.is_empty()).then_some(tag),
                    );
                    power_plant_index += 1;
                    return Some(AuthoredOrderedUpgrade::PowerPlant(AuthoredPowerPlantUpgrade {
                        executed_key,
                        data,
                    }));
                }
                if module.name.as_str().eq_ignore_ascii_case("RadarUpgrade") {
                    let data = upgrade_data_view::<gamelogic::object::upgrade::RadarUpgradeModuleData>(
                        &module.data,
                    )?;
                    let state_index = radar_index;
                    radar_index += 1;
                    return Some(AuthoredOrderedUpgrade::Radar(AuthoredRadarUpgrade {
                        state_index,
                        data,
                    }));
                }
                if module.name.as_str().eq_ignore_ascii_case("PassengersFireUpgrade") {
                    let data = upgrade_data_view::<
                        gamelogic::object::upgrade::passengers_fire_upgrade::PassengersFireUpgradeModuleData,
                    >(&module.data)?;
                    return Some(AuthoredOrderedUpgrade::PassengersFire(
                        AuthoredPassengersFireUpgrade { source_index, data }
                    ));
                }
                if module.name.as_str().eq_ignore_ascii_case("MaxHealthUpgrade") {
                    let data = upgrade_data_view::<
                        gamelogic::object::upgrade::max_health_upgrade::MaxHealthUpgradeModuleData,
                    >(&module.data)?;
                    return Some(AuthoredOrderedUpgrade::MaxHealth(
                        AuthoredMaxHealthUpgrade { source_index, data }
                    ));
                }
                if module.name.as_str().eq_ignore_ascii_case("WeaponSetUpgrade") {
                    if !module.data.is::<gamelogic::object::upgrade::weapon_set_upgrade::WeaponSetUpgradeModuleData>() { return None; }
                    return Some(AuthoredOrderedUpgrade::Weapon(AuthoredWeaponUpgrade {
                        source_index,
                        kind: AuthoredWeaponUpgradeKind::Set,
                        mux: ModuleDataView::new(module.data.clone(), |data| &data.as_any().downcast_ref::<gamelogic::object::upgrade::weapon_set_upgrade::WeaponSetUpgradeModuleData>().expect("weapon set data").upgrade_mux_data),
                    }));
                }
                if module.name.as_str().eq_ignore_ascii_case("WeaponBonusUpgrade") {
                    if !module.data.is::<gamelogic::object::upgrade::weapon_bonus_upgrade::WeaponBonusUpgradeModuleData>() { return None; }
                    return Some(AuthoredOrderedUpgrade::Weapon(AuthoredWeaponUpgrade {
                        source_index,
                        kind: AuthoredWeaponUpgradeKind::Bonus,
                        mux: ModuleDataView::new(module.data.clone(), |data| &data.as_any().downcast_ref::<gamelogic::object::upgrade::weapon_bonus_upgrade::WeaponBonusUpgradeModuleData>().expect("weapon bonus data").upgrade_mux_data),
                    }));
                }
                None
            })
            .collect();
        if let Some(object) = self.objects.get_mut(&object_id) {
            object
                .object_creation_upgrade_executed
                .resize(object_creation_index, false);
            object
                .status_bits_upgrade_executed
                .resize(status_bits_index, false);
            object
                .cost_modifier_upgrade_executed
                .resize(cost_modifier_index, false);
            object.radar_upgrade_executed.resize(radar_index, false);
            object
                .radar_upgrade_contributors
                .resize(radar_index, Vec::new());
        }
        modules
    }

    pub(super) fn prepare_authored_upgrade_pass<'a>(
        &mut self,
        object_id: ObjectId,
        pending_upgrade: Option<&'a str>,
    ) -> Option<AuthoredUpgradePass<'a>> {
        let object = self.objects.get(&object_id)?;
        if object.under_construction_status()
            || object.status.destroyed
            || self.player_owner_for_host_object(object).is_none()
        {
            return None;
        }
        let birth = std::sync::Arc::downgrade(&object.visual_draw_identity_token);
        Some(AuthoredUpgradePass {
            object_id,
            birth,
            pending_upgrade,
            modules: self.authored_ordered_upgrade_modules(object_id),
            mask: self.combined_object_and_player_upgrade_mask(object_id, pending_upgrade),
        })
    }

    /// Run the typed UpgradeMux families in declaration order. Unmigrated
    /// families still run through the residual path.
    pub(super) fn execute_authored_ordered_upgrade_modules(
        &mut self,
        pass: AuthoredUpgradePass<'_>,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let AuthoredUpgradePass {
            object_id,
            birth,
            pending_upgrade,
            modules,
            mask: key_mask,
        } = pass;
        for module in modules {
            if !self.object_birth_matches(object_id, &birth) {
                return Ok(());
            }
            let already_executed = match &module {
                AuthoredOrderedUpgrade::ObjectCreation(module) => self
                    .objects
                    .get(&object_id)
                    .and_then(|object| {
                        object
                            .object_creation_upgrade_executed
                            .get(module.state_index)
                    })
                    .copied()
                    .unwrap_or(false),
                AuthoredOrderedUpgrade::StatusBits(module) => self
                    .objects
                    .get(&object_id)
                    .and_then(|object| object.status_bits_upgrade_executed.get(module.state_index))
                    .copied()
                    .unwrap_or(false),
                AuthoredOrderedUpgrade::CostModifier(module) => self
                    .objects
                    .get(&object_id)
                    .and_then(|object| {
                        object
                            .cost_modifier_upgrade_executed
                            .get(module.state_index)
                    })
                    .copied()
                    .unwrap_or(false),
                AuthoredOrderedUpgrade::PowerPlant(module) => self
                    .objects
                    .get(&object_id)
                    .is_some_and(|object| object.applied_upgrades.contains(&module.executed_key)),
                AuthoredOrderedUpgrade::Radar(module) => self
                    .objects
                    .get(&object_id)
                    .and_then(|object| object.radar_upgrade_executed.get(module.state_index))
                    .copied()
                    .unwrap_or(false),
                AuthoredOrderedUpgrade::ReplaceObject(module) => self
                    .objects
                    .get(&object_id)
                    .is_some_and(|object| object.applied_upgrades.contains(&module.executed_key())),
                AuthoredOrderedUpgrade::Visual(module) => self
                    .objects
                    .get(&object_id)
                    .is_some_and(|object| object.applied_upgrades.contains(&module.executed_key())),
                AuthoredOrderedUpgrade::Armor(module) => self
                    .objects
                    .get(&object_id)
                    .is_some_and(|object| object.applied_upgrades.contains(&module.executed_key())),
                AuthoredOrderedUpgrade::CommandSet(module) => self
                    .objects
                    .get(&object_id)
                    .is_some_and(|object| object.applied_upgrades.contains(&module.executed_key())),
                AuthoredOrderedUpgrade::PassengersFire(module) => self
                    .objects
                    .get(&object_id)
                    .is_some_and(|object| object.applied_upgrades.contains(&module.executed_key())),
                AuthoredOrderedUpgrade::MaxHealth(module) => self
                    .objects
                    .get(&object_id)
                    .is_some_and(|object| object.applied_upgrades.contains(&module.executed_key())),
                AuthoredOrderedUpgrade::Weapon(module) => self
                    .objects
                    .get(&object_id)
                    .is_some_and(|object| object.applied_upgrades.contains(&module.executed_key())),
            };
            if already_executed || !module.with_mux(|mux| self.upgrade_mux_would_run(mux, key_mask))
            {
                continue;
            }
            let fx_list =
                module.with_mux(|mux| mux.fx_list_upgrade.as_ref().map(|fx| fx.name().to_owned()));
            if let Some(fx_list) = fx_list {
                let _ = self.dispatch_fx_list_at_host_object_with_effects(
                    &fx_list,
                    object_id,
                    None,
                    presentation_effects,
                );
                // FX nuggets can synchronously execute gameplay callbacks.
                // Never apply this module's removals/OCL to a replacement
                // admitted under the same ObjectId.
                if !self.object_birth_matches(object_id, &birth) {
                    return Ok(());
                }
            }
            let removal_names = module.with_mux(|mux| mux.removal_upgrade_names.clone());
            for removed in &removal_names {
                self.remove_object_upgrade_with_receiving_rules(object_id, removed.as_str());
            }
            let result = match &module {
                AuthoredOrderedUpgrade::ReplaceObject(module) => {
                    let target = module.data.read().replace_object_name().as_str().to_owned();
                    self.replace_object_with_effects(
                        object_id,
                        &birth,
                        &target,
                        false,
                        interpreter,
                        presentation_effects,
                    )
                }
                AuthoredOrderedUpgrade::Visual(module) => {
                    match &module.effect {
                        VisualUpgradeEffect::ModelCondition(data) => {
                            if let Some(object) = self
                                .objects
                                .get_mut(&object_id)
                                .filter(|object| object.has_drawable)
                            {
                                object.model_condition_bits |= data.read().condition_flag().bits();
                            }
                        }
                        VisualUpgradeEffect::SubObjects(data) => {
                            self.apply_sub_object_upgrade_effect(object_id, &data.read());
                        }
                    }
                    Ok(())
                }
                AuthoredOrderedUpgrade::Armor(module) => {
                    use crate::game_logic::host_upgrades::UPGRADE_AMERICA_CHEMICAL_SUITS;
                    if let Some(object) = self.objects.get_mut(&object_id) {
                        if !object.is_inactive_body() {
                            object.set_armor_set_player_upgrade(true);
                        }
                        // ArmorUpgrade.cpp:76–80 tests the authored trigger list.
                        if module
                            .data
                            .read()
                            .upgrade_mux_data
                            .is_triggered_by(UPGRADE_AMERICA_CHEMICAL_SUITS)
                        {
                            object.apply_chemical_suit_decal();
                        }
                        if let Some(upgrade) = pending_upgrade {
                            self.upgrade_module_residuals.record_armor_set(upgrade);
                        }
                    }
                    Ok(())
                }
                AuthoredOrderedUpgrade::CommandSet(module) => {
                    let data = module.data.read();
                    // CommandSetUpgrade.cpp:47–78 reads the completed masks at
                    // invocation, after preceding callbacks and this module's removals.
                    let use_alternative = self
                        .upgrade_template(data.trigger_alt().as_str())
                        .is_some_and(|trigger| {
                            self.combined_object_and_player_upgrade_mask(object_id, None)
                                .test_for_any(trigger.get_mask())
                        });
                    let name = if use_alternative {
                        data.command_set_alt()
                    } else {
                        data.command_set_name()
                    };
                    if let Some(object) = self.objects.get_mut(&object_id) {
                        object.set_command_set_override(
                            (!name.is_empty()).then(|| name.as_str().to_owned()),
                        );
                        self.replace_grant_command_upgrades
                            .record_command_set(name.as_str());
                        if let Some(ui) = presentation_effects.ui.as_deref_mut() {
                            ui.mark_control_bar_dirty();
                        }
                    }
                    Ok(())
                }
                AuthoredOrderedUpgrade::StatusBits(module) => {
                    let names =
                        crate::game_logic::host_enum_table_residual::OBJECT_STATUS_BIT_NAME_LIST;
                    let set: Vec<_> = names
                        .iter()
                        .enumerate()
                        .filter_map(|(i, name)| {
                            (i > 0 && module.data.read().status_to_set().bits() & (1u64 << i) != 0)
                                .then_some(*name)
                        })
                        .collect();
                    let clear: Vec<_> = names
                        .iter()
                        .enumerate()
                        .filter_map(|(i, name)| {
                            (i > 0
                                && module.data.read().status_to_clear().bits() & (1u64 << i) != 0)
                                .then_some(*name)
                        })
                        .collect();
                    if let Some((set_count, clear_count)) = self
                        .apply_object_status_bits_upgrade_with_effects(
                            object_id,
                            &set,
                            &clear,
                            presentation_effects,
                        )
                    {
                        self.status_bits_upgrade_reg
                            .record_apply(set_count, clear_count);
                    }
                    Ok(())
                }
                AuthoredOrderedUpgrade::CostModifier(module) => {
                    if let Some(owner_id) = self
                        .objects
                        .get(&object_id)
                        .and_then(|object| self.player_owner_for_host_object(object))
                    {
                        if let Some(player) = self.players.get_mut(&owner_id) {
                            let data = module.data.read();
                            player.add_authored_kind_of_production_cost_change(
                                data.kind_of(),
                                data.percentage(),
                            );
                            if let Some(upgrade) = pending_upgrade {
                                self.upgrade_module_residuals.record_cost(upgrade);
                            }
                        }
                    }
                    Ok(())
                }
                AuthoredOrderedUpgrade::PowerPlant(_) => {
                    if let Some(object) = self.objects.get(&object_id) {
                        let template = object.get_template();
                        let bonus = template.energy_bonus.unwrap_or(0);
                        let animate_rods = template.power_plant_update.is_some();
                        self.change_object_power_bonus(object_id, bonus);
                        if animate_rods {
                            self.begin_power_plant_rods_extend(object_id);
                        }
                    }
                    Ok(())
                }
                AuthoredOrderedUpgrade::Radar(module) => {
                    let proof = module.data.read().is_disable_proof();
                    self.grant_radar_upgrade_slot(object_id, module.state_index, proof);
                    Ok(())
                }
                AuthoredOrderedUpgrade::PassengersFire(_) => {
                    let owner = self.redirected_containment_owner(object_id);
                    if let Some(contain) = self.objects.get_mut(&owner) {
                        if contain.thing().template.contain_module.kind
                            != crate::game_logic::ContainModuleKind::None
                        {
                            contain.passengers_allowed_to_fire = true;
                            self.passengers_fire_upgrade_reg.record_apply(1);
                        }
                    }
                    Ok(())
                }
                AuthoredOrderedUpgrade::MaxHealth(module) => {
                    if let Some(object) = self.objects.get_mut(&object_id) {
                        object.apply_authored_max_health_upgrade(&module.data.read());
                        self.health_events
                            .record_applied_heal(object_id, object.health.current);
                    }
                    Ok(())
                }
                AuthoredOrderedUpgrade::Weapon(module) => {
                    match module.kind {
                        AuthoredWeaponUpgradeKind::Set => {
                            if let Some(object) = self.objects.get_mut(&object_id) {
                                // WeaponSetUpgrade.cpp:27-32: a selection
                                // change rebinds concrete Weapons immediately.
                                object.set_weapon_condition_at_frame(3, true, self.frame);
                            }
                            if let Some(upgrade) = pending_upgrade {
                                self.upgrade_module_residuals.record_weapon_set(upgrade);
                            }
                        }
                        AuthoredWeaponUpgradeKind::Bonus => {
                            let changed = self.objects.get_mut(&object_id).is_some_and(|object| {
                                let changed = !object.weapon_bonus_player_upgrade;
                                object.set_weapon_bonus_player_upgrade(true);
                                changed
                            });
                            if changed {
                                // Object.cpp:4650-4672: notify immediately on
                                // the changed bit, before the next module/OCL.
                                self.notify_owned_weapon_bonus_change(object_id);
                            }
                            if let Some(upgrade) = pending_upgrade {
                                self.upgrade_module_residuals.record_weapon_bonus(upgrade);
                            }
                        }
                    }
                    Ok(())
                }
                AuthoredOrderedUpgrade::ObjectCreation(module) => {
                    let (ocl_name, module_drone_kind) = {
                        let data = module.data.read();
                        let ocl_name = data
                            .ocl_name
                            .as_ref()
                            .map(|name| name.as_str().trim())
                            .filter(|name| !name.is_empty() && !name.eq_ignore_ascii_case("None"))
                            .map(str::to_owned);
                        let kind = pending_upgrade
                            .and_then(crate::game_logic::host_slave_drones::SlaveDroneKind::from_upgrade_name)
                            .or_else(|| data.upgrade_mux_data.activation_upgrade_names.iter()
                                .chain(&data.upgrade_mux_data.trigger_upgrade_names)
                                .find_map(|name| crate::game_logic::host_slave_drones::SlaveDroneKind::from_upgrade_name(name.as_str())));
                        (ocl_name, kind)
                    };
                    if let Some(ocl_name) = ocl_name {
                        let slave_kind = module_drone_kind.filter(|kind| {
                            ocl_name.eq_ignore_ascii_case(
                                crate::game_logic::host_slave_drones::drone_ocl_name(*kind),
                            )
                        });
                        self.apply_object_creation_upgrade_ocl_with_interpreter_and_effects(
                            object_id,
                            &ocl_name,
                            slave_kind,
                            &birth,
                            interpreter,
                            presentation_effects,
                        )
                    } else {
                        Ok(())
                    }
                }
            };
            if self.object_birth_matches(object_id, &birth) {
                if let Some(object) = self.objects.get_mut(&object_id) {
                    match module {
                        AuthoredOrderedUpgrade::ObjectCreation(module) => {
                            if let Some(executed) = object
                                .object_creation_upgrade_executed
                                .get_mut(module.state_index)
                            {
                                *executed = true;
                            }
                        }
                        AuthoredOrderedUpgrade::StatusBits(module) => {
                            if let Some(executed) = object
                                .status_bits_upgrade_executed
                                .get_mut(module.state_index)
                            {
                                *executed = true;
                            }
                        }
                        AuthoredOrderedUpgrade::CostModifier(module) => {
                            if let Some(executed) = object
                                .cost_modifier_upgrade_executed
                                .get_mut(module.state_index)
                            {
                                *executed = true;
                            }
                        }
                        AuthoredOrderedUpgrade::PowerPlant(module) => {
                            object.applied_upgrades.insert(module.executed_key);
                        }
                        AuthoredOrderedUpgrade::Radar(module) => {
                            object.radar_upgrade_executed[module.state_index] = true;
                        }
                        AuthoredOrderedUpgrade::ReplaceObject(module) => {
                            // UpgradeMux persists this per-module latch even
                            // when the authored replacement template is absent.
                            object.applied_upgrades.insert(module.executed_key());
                        }
                        AuthoredOrderedUpgrade::Visual(module) => {
                            object.applied_upgrades.insert(module.executed_key());
                        }
                        AuthoredOrderedUpgrade::Armor(module) => {
                            object.applied_upgrades.insert(module.executed_key());
                        }
                        AuthoredOrderedUpgrade::CommandSet(module) => {
                            object.applied_upgrades.insert(module.executed_key());
                        }
                        AuthoredOrderedUpgrade::PassengersFire(module) => {
                            object.applied_upgrades.insert(module.executed_key());
                        }
                        AuthoredOrderedUpgrade::MaxHealth(module) => {
                            object.applied_upgrades.insert(module.executed_key());
                        }
                        AuthoredOrderedUpgrade::Weapon(module) => {
                            object.applied_upgrades.insert(module.executed_key());
                        }
                    }
                }
            }
            result?;
        }
        Ok(())
    }

    /// Re-evaluate the combined mask after construction completion. C++
    /// `updateUpgradeModules` sees object upgrades already present in the mask,
    /// even when they were granted while `UNDER_CONSTRUCTION` blocked modules.
    pub(crate) fn update_authored_upgrade_modules_with_effects(
        &mut self,
        object_id: ObjectId,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let Some(pass) = self.prepare_authored_upgrade_pass(object_id, None) else {
            return Ok(());
        };
        self.execute_authored_ordered_upgrade_modules(pass, interpreter, presentation_effects)
    }

    fn combined_object_and_player_upgrade_mask(
        &self,
        object_id: ObjectId,
        pending_upgrade: Option<&str>,
    ) -> gamelogic::upgrade::UpgradeMask {
        let Some(object) = self.objects.get(&object_id) else {
            return gamelogic::upgrade::UpgradeMask::none();
        };
        // C++ Object::updateUpgradeModules combines two separate masks:
        // completed upgrades on this Object and completed upgrades on its
        // current controlling Player. Object grant names are explicit
        // provenance: C++ Object::giveUpgrade records any supplied template's
        // mask, even when that template is Type=PLAYER. Keep object upgrades
        // available when there is no controlling Player.
        let mut mask = object
            .completed_object_upgrades
            .iter()
            .filter_map(|name| self.upgrade_template(name))
            .fold(gamelogic::upgrade::UpgradeMask::none(), |mask, template| {
                mask | template.get_mask()
            });
        if let Some(owner_id) = self.player_owner_for_host_object(object) {
            if let Some(player) = self.players.get(&owner_id) {
                mask |= player
                    .completed_upgrades
                    .iter()
                    .filter_map(|name| self.upgrade_template(name))
                    .fold(gamelogic::upgrade::UpgradeMask::none(), |mask, template| {
                        mask | template.get_mask()
                    });
            }
        }
        if let Some(pending) = pending_upgrade {
            if let Some(template) = self.upgrade_template(pending) {
                mask |= template.get_mask();
            }
        }
        mask
    }

    /// Main's raw Object tag mutator has no receiving world argument. Route
    /// production removals through this owner boundary so UpgradeMux reset uses
    /// this session's definitions rather than the process UpgradeCenter.
    pub(crate) fn remove_object_upgrade_with_receiving_rules(
        &mut self,
        object_id: ObjectId,
        upgrade: &str,
    ) -> bool {
        let removed_mask = self
            .upgrade_template(upgrade)
            .map(|template| template.get_mask().bits())
            .unwrap_or(0);
        let modules = self.authored_ordered_upgrade_modules(object_id);
        let activation_masks: Vec<_> = modules
            .iter()
            .map(|module| module.with_mux(|mux| self.upgrade_activation_mask(mux)))
            .collect();
        let mut tags_to_remove = self
            .objects
            .get(&object_id)
            .map(|object| {
                object
                    .applied_upgrades
                    .iter()
                    .filter(|name| {
                        self.upgrade_template(name.as_str())
                            .is_some_and(|template| template.get_mask().bits() & removed_mask != 0)
                    })
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if !tags_to_remove
            .iter()
            .any(|name| name.eq_ignore_ascii_case(upgrade))
        {
            tags_to_remove.push(upgrade.to_owned());
        }
        let mut completed_names_to_remove: Vec<_> = self
            .objects
            .get(&object_id)
            .map(|object| {
                object
                    .completed_object_upgrades
                    .iter()
                    .filter(|name| {
                        name.eq_ignore_ascii_case(upgrade)
                            || (removed_mask != 0
                                && self.upgrade_template(name).is_some_and(|template| {
                                    template.get_mask().bits() & removed_mask != 0
                                }))
                    })
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        if !completed_names_to_remove
            .iter()
            .any(|name| name.eq_ignore_ascii_case(upgrade))
        {
            completed_names_to_remove.push(upgrade.to_owned());
        }
        let Some(object) = self.objects.get_mut(&object_id) else {
            return false;
        };
        let mut removed = false;
        for name in tags_to_remove {
            removed |= object.remove_upgrade_tag(&name);
        }
        for name in completed_names_to_remove {
            removed |= object.completed_object_upgrades.remove(&name);
        }
        if removed_mask != 0 {
            for (index, activation) in activation_masks.iter().enumerate() {
                if activation & removed_mask != 0 {
                    match modules.get(index) {
                        Some(AuthoredOrderedUpgrade::ObjectCreation(module)) => {
                            if let Some(executed) = object
                                .object_creation_upgrade_executed
                                .get_mut(module.state_index)
                            {
                                *executed = false;
                            }
                        }
                        Some(AuthoredOrderedUpgrade::StatusBits(module)) => {
                            if let Some(executed) = object
                                .status_bits_upgrade_executed
                                .get_mut(module.state_index)
                            {
                                *executed = false;
                            }
                        }
                        Some(AuthoredOrderedUpgrade::CostModifier(module)) => {
                            if let Some(executed) = object
                                .cost_modifier_upgrade_executed
                                .get_mut(module.state_index)
                            {
                                *executed = false;
                            }
                        }
                        Some(AuthoredOrderedUpgrade::PowerPlant(module)) => {
                            object.applied_upgrades.remove(&module.executed_key);
                        }
                        Some(AuthoredOrderedUpgrade::Radar(module)) => {
                            object.radar_upgrade_executed[module.state_index] = false;
                        }
                        Some(AuthoredOrderedUpgrade::ReplaceObject(module)) => {
                            object.applied_upgrades.remove(&module.executed_key());
                        }
                        Some(AuthoredOrderedUpgrade::Visual(module)) => {
                            object.applied_upgrades.remove(&module.executed_key());
                        }
                        Some(AuthoredOrderedUpgrade::Armor(module)) => {
                            // Reset leaves the ActiveBody's flag and the Drawable's decal.
                            object.applied_upgrades.remove(&module.executed_key());
                        }
                        Some(AuthoredOrderedUpgrade::CommandSet(module)) => {
                            // UpgradeMux::resetUpgrade leaves the published command set.
                            object.applied_upgrades.remove(&module.executed_key());
                        }
                        Some(AuthoredOrderedUpgrade::PassengersFire(module)) => {
                            object.applied_upgrades.remove(&module.executed_key());
                        }
                        Some(AuthoredOrderedUpgrade::MaxHealth(module)) => {
                            // Object.cpp:4491-4503 resets execution, not health.
                            object.applied_upgrades.remove(&module.executed_key());
                        }
                        Some(AuthoredOrderedUpgrade::Weapon(module)) => {
                            object.applied_upgrades.remove(&module.executed_key());
                        }
                        None => {}
                    }
                }
            }
        }
        removed
    }

    fn upgrade_activation_mask(
        &self,
        data: &gamelogic::upgrade::modules::upgrade_mux::UpgradeMuxData,
    ) -> u128 {
        data.activation_upgrade_names
            .iter()
            .chain(&data.trigger_upgrade_names)
            .filter_map(|name| self.upgrade_template(name.as_str()))
            .fold(0u128, |mask, template| mask | template.get_mask().bits())
    }

    /// SubObjectsUpgrade.cpp:63–108 rechecks live conflicts after the mux's
    /// FX and removals. Reveal also calls this effect without running the mux.
    fn apply_sub_object_upgrade_effect(
        &mut self,
        id: ObjectId,
        data: &SubObjectsUpgradeModuleData,
    ) {
        let conflicts = self.upgrade_conflict_mask(&data.upgrade_mux_data);
        if conflicts.any()
            && self
                .combined_object_and_player_upgrade_mask(id, None)
                .test_for_any(conflicts)
        {
            return;
        }
        let Some(object) = self
            .objects
            .get_mut(&id)
            .filter(|object| object.has_drawable)
        else {
            return;
        };
        let shown: Vec<_> = data
            .show_sub_object_names()
            .iter()
            .map(|name| name.as_str())
            .collect();
        for name in &shown {
            object.sub_object_visibility.set_shown(name, true);
        }
        for name in data.hide_sub_object_names() {
            object.sub_object_visibility.set_shown(name.as_str(), false);
        }
        if !shown.is_empty() {
            self.sub_objects_upgrades.record(&shown);
        }
    }

    /// Object::forceRefreshSubObjectUpgradeStatus on the replacement Drawable.
    pub(in crate::game_logic) fn refresh_sub_objects_after_disguise(&mut self, id: ObjectId) {
        if !self.has_admitted_upgrade_roster(id) {
            return;
        }
        let modules = self.authored_ordered_upgrade_modules(id);
        let Some(object) = self.objects.get_mut(&id) else {
            return;
        };
        object.sub_object_visibility = Default::default();
        for module in modules {
            let AuthoredOrderedUpgrade::Visual(module) = module else {
                continue;
            };
            if !self.objects[&id]
                .applied_upgrades
                .contains(&module.executed_key())
            {
                continue;
            }
            if let VisualUpgradeEffect::SubObjects(data) = module.effect {
                self.apply_sub_object_upgrade_effect(id, &data.read());
            }
        }
    }

    fn upgrade_conflict_mask(
        &self,
        data: &gamelogic::upgrade::modules::upgrade_mux::UpgradeMuxData,
    ) -> gamelogic::upgrade::UpgradeMask {
        use gamelogic::upgrade::UpgradeMask;
        data.conflicting_upgrade_names
            .iter()
            .filter_map(|name| self.upgrade_template(name.as_str()))
            .fold(UpgradeMask::none(), |mask, template| {
                mask | template.get_mask()
            })
    }

    fn upgrade_mux_would_run(
        &self,
        data: &gamelogic::upgrade::modules::upgrade_mux::UpgradeMuxData,
        key_mask: gamelogic::upgrade::UpgradeMask,
    ) -> bool {
        use gamelogic::upgrade::UpgradeMask;
        let activation = data
            .activation_upgrade_names
            .iter()
            .chain(&data.trigger_upgrade_names)
            .filter_map(|name| self.upgrade_template(name.as_str()))
            .fold(UpgradeMask::none(), |mask, template| {
                mask | template.get_mask()
            });
        let conflicts = self.upgrade_conflict_mask(data);
        activation.any()
            && key_mask.any()
            && !key_mask.test_for_any(conflicts)
            && if data.requires_all_triggers {
                key_mask.test_for_all(activation)
            } else {
                key_mask.test_for_any(activation)
            }
    }
}

#[cfg(test)]
#[path = "object_creation_upgrade_birth_tests.rs"]
mod object_creation_upgrade_birth_tests;
