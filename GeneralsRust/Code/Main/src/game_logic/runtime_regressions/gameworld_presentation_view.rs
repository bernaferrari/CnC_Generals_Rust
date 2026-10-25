//! Executable command and lifecycle regressions.

#[test]
fn presentation_view_from_shadow_reports_created_host_object() {
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::gameworld_shadow::{GameWorldShadow, presentation_view_from_shadow};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("GwViewRanger186");
    t.set_health(120.0);
    t.add_kind_of(KindOf::Infantry);
    t.add_kind_of(KindOf::Selectable);
    logic.templates.insert("GwViewRanger186".into(), t);
    let id = logic
        .create_object("GwViewRanger186", Team::USA, Vec3::new(8.0, 0.0, 16.0))
        .expect("create host object");

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let view = presentation_view_from_shadow(&shadow, 0);
    assert!(
        !view.entities.is_empty(),
        "presentation_view_from_shadow must observe synced host objects"
    );
    assert!(
        view.entities
            .iter()
            .any(|e| e.id == id.0 || e.template.contains("GwViewRanger186")),
        "observe-path view must include the created unit"
    );
    assert!(
        view.entities.len() >= 1,
        "entity count is the host residual stamped after shadow session"
    );
}

fn gameworld_presentation_view() -> bool {
    use crate::game_logic::{
        DEFAULT_SKIRMISH_MAP_WAVE169, GameLogic, GameMode, LONE_EAGLE_MAP_WAVE169,
        resolve_retail_map_path,
    };
    use crate::gameworld_shadow::{GameWorldShadow, presentation_view_from_shadow};
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};

    // Soft-ok when MapsZH absent (CI without retail maps).
    let map_name = if resolve_retail_map_path(LONE_EAGLE_MAP_WAVE169).is_some() {
        LONE_EAGLE_MAP_WAVE169
    } else if resolve_retail_map_path(DEFAULT_SKIRMISH_MAP_WAVE169).is_some() {
        DEFAULT_SKIRMISH_MAP_WAVE169
    } else {
        // Still prove empty-world view API.
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("GwPresView186");
        let _ = apply_skirmish_config(&mut logic, &cfg);
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let view = presentation_view_from_shadow(&shadow, 0);
        if !view.entities.is_empty() {
            return false;
        }
        let mut template = crate::game_logic::ThingTemplate::new("ReceivingPresentation186");
        template.set_health(120.0);
        template.add_kind_of(crate::game_logic::KindOf::Infantry);
        logic
            .templates
            .insert("ReceivingPresentation186".into(), template);
        let Some(id) = logic.create_object(
            "ReceivingPresentation186",
            crate::game_logic::Team::USA,
            glam::Vec3::new(8.0, 0.0, 16.0),
        ) else {
            return false;
        };
        shadow.sync_from_host(&logic);
        let view = presentation_view_from_shadow(&shadow, 0);
        return view.entities.iter().any(|entity| entity.id == id.0)
            && logic
                .host_object(id)
                .is_some_and(|object| object.health.current == 120.0);
    };

    let mut logic = GameLogic::new();
    logic.start_new_game(GameMode::Skirmish);
    let cfg = golden_skirmish_config("GwPresView186");
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        return false;
    }
    let path = resolve_retail_map_path(map_name);
    let loaded = match path {
        Some(p) => {
            let s = p.to_string_lossy();
            logic.load_map(s.as_ref()) || logic.load_map(map_name)
        }
        None => logic.load_map(map_name),
    };
    if !loaded || logic.get_objects().registered_is_empty() {
        return false;
    }
    logic.update();

    let mut shadow = GameWorldShadow::new(4096);
    shadow.sync_from_host(&logic);
    let view = presentation_view_from_shadow(&shadow, 0);
    if view.entities.is_empty() {
        return false;
    }
    // Entity count should track host objects at least partially.
    if view.entities.len() < 1 {
        return false;
    }

    true
}

#[test]
fn gameworld_presentation_view_runs() {
    assert!(gameworld_presentation_view());
}
