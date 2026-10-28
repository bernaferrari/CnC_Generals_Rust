//! Per-operation Body particle updates using the receiving world.
/// Shared synchronous Body auto-particle replacement used by the native view
/// operation and low-level Body callbacks that do not otherwise need a view.
pub(crate) fn update_body_auto_particles(
    particles: &mut crate::game_logic::combat_particles::CombatParticleRegistry,
    object: &crate::game_logic::Object,
    body_state: crate::game_logic::host_enum_table_residual::HostBodyDamageType,
    frame: u32,
    player: Option<u32>,
    shroud: &std::sync::Arc<std::sync::Mutex<gamelogic::system::shroud_manager::ShroudManager>>,
    particle_owner: &mut dyn crate::game_logic::combat_particles::ParticleBindingAccess,
    random: &mut game_engine::common::random_value::RandomState,
) {
    let position = object.get_position();
    // NativeVisualState is emitted before Object::rebuild_model_condition_bits
    // to preserve ActiveBody's Drawable callback boundary. Supply its target
    // damage-state bits when resolving the model's FIRE/SMOKE bones.
    let visual_state = if !object.status.under_construction || object.status.destroyed {
        body_state
    } else {
        crate::game_logic::host_enum_table_residual::HostBodyDamageType::Pristine
    };
    let model_bits = crate::game_logic::host_enum_table_residual::host_apply_body_damage_model_bits(
        object.model_condition_bits,
        visual_state,
    );
    let model = object
        .thing()
        .template
        .model_name_for_conditions(model_bits);
    let scale = object.thing().template.asset_scale;
    let yaw = object.get_orientation();
    let aflame = object.has_object_status_bit("AFLAME")
        || object
            .fire_spread
            .as_ref()
            .is_some_and(|spread| spread.is_aflame());
    let pose = crate::game_logic::combat_particles::BodyAutoParticlePose::with_visibility(
        &model,
        scale,
        yaw,
        crate::game_logic::GameLogic::host_fx_visibility_for(player, shroud, object.id.0),
    );
    particles.replace_body_auto_particles(
        object.id,
        position,
        frame,
        body_state.ordinal(),
        aflame,
        pose,
        particle_owner,
        random,
    );
}

/// Short synchronous host-only lease used by Body callbacks when no native
/// view is compiled. It mutates the same receiving registry as the view path.
pub(crate) struct HeadlessBodyParticleUpdate<'a> {
    pub particles: &'a mut crate::game_logic::combat_particles::CombatParticleRegistry,
    pub random: &'a mut game_engine::common::random_value::RandomState,
    pub player: Option<u32>,
    pub shroud:
        &'a std::sync::Arc<std::sync::Mutex<gamelogic::system::shroud_manager::ShroudManager>>,
    pub frame: u32,
}

impl HeadlessBodyParticleUpdate<'_> {
    pub(crate) fn update(
        &mut self,
        object: &crate::game_logic::Object,
        body_state: crate::game_logic::host_enum_table_residual::HostBodyDamageType,
    ) {
        update_body_auto_particles(
            self.particles,
            object,
            body_state,
            self.frame,
            self.player,
            self.shroud,
            &mut (),
            self.random,
        );
    }
}
