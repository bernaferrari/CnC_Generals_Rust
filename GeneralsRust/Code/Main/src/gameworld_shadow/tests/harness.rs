//! Helpers that exercise the installed Main body directly.

/// Run a libtest case that requires process isolation in a fresh process, rather than
/// mutating the environment shared by the test runner and native libraries.
/// Returns true in the parent after the exact child test has passed.
pub(crate) fn isolated_shadow_case(module: &str, test: &str) -> bool {
    const MARKER: &str = "GENERALS_ISOLATED_SHADOW_TEST";
    let module = module.split_once("::").expect("test module path").1;
    let name = format!("{module}::{test}");
    if std::env::var(MARKER).as_deref() == Ok(name.as_str()) {
        return false;
    }

    let output = std::process::Command::new(std::env::current_exe().expect("test executable"))
        .args(["--exact", &name, "--nocapture", "--test-threads=1"])
        .env(MARKER, &name)
        .output()
        .expect("launch isolated shadow test");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success()
            && stdout.contains("running 1 test")
            && stdout.contains("1 passed; 0 failed"),
        "isolated test {name} failed or did not run: {}\n{stdout}\n{stderr}",
        output.status
    );
    true
}

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
