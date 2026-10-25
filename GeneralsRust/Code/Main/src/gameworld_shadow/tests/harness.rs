//! Helpers that exercise the installed Main body directly.

/// Exercise the installed Main body's real synchronous damage operation.
pub(crate) fn perform_owner_damage(
    logic: &mut crate::game_logic::GameLogic,
    id: crate::game_logic::ObjectId,
    amount: f32,
    source: Option<crate::game_logic::ObjectId>,
) {
    use crate::game_logic::{
        combat::DamageType, host_usa_pilot::HostDeathType, object::DamageHitContext,
    };
    let kind = DamageType::Explosive;
    let context = DamageHitContext::new(source.and_then(|id| logic.host_object(id)), None, kind);
    let _ = logic.apply_owned_damage(
        id,
        amount,
        source,
        kind,
        HostDeathType::from_host_damage_type(kind),
        None,
        &context,
    );
}

/// A genuine body heal commits immediately; explicit script absolute-health
/// control handles reductions without inventing a pending mutation channel.
pub(crate) fn perform_owner_health(
    logic: &mut crate::game_logic::GameLogic,
    id: crate::game_logic::ObjectId,
    health: f32,
) {
    let Some(current) = logic.host_object(id).map(|o| o.health.current) else {
        return;
    };
    if health > current {
        let body_frame = logic.frame;
        let (object, observations) = logic.host_object_and_health_events_mut(id).unwrap();
        object.heal(health - current, observations, body_frame);
    } else {
        logic.set_health_absolute_authority_aware(id, health);
    }
}
