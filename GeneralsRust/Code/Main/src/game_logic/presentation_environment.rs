//! Presentation environment.

/// Wave 466: pipeline env freeze from host + optional GameWorld shadow.
/// Mirrors `host_ensure_presentation_env_for_hints` when the pipeline has no frame.
pub fn seed_presentation_env_frame_from_host_and_shadow(
    logic: &mut crate::game_logic::GameLogic,
    local_player_id: u32,
    shadow: Option<&crate::gameworld_shadow::GameWorldShadow>,
) -> crate::presentation_frame::PresentationFrame {
    crate::presentation_frame::PresentationFrame::build_for_engine(logic, local_player_id, shadow)
}

/// Engine-cache variant of the env seed. The payload is immutable and owned by
/// the presentation frame, so this preserves the no-live-GameLogic renderer
/// boundary while avoiding a full height/blend clone for every seed.
pub fn seed_presentation_env_frame_from_host_and_shadow_with_runtime_heightmap(
    logic: &mut crate::game_logic::GameLogic,
    local_player_id: u32,
    shadow: Option<&crate::gameworld_shadow::GameWorldShadow>,
    runtime_heightmap: Option<
        std::sync::Arc<crate::presentation_frame::PresentationRuntimeHeightmap>,
    >,
) -> crate::presentation_frame::PresentationFrame {
    crate::presentation_frame::PresentationFrame::build_for_engine_with_runtime_heightmap(
        logic,
        local_player_id,
        shadow,
        runtime_heightmap,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_presentation_env_frame_includes_gameworld_shadow_objects() {
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
        use crate::gameworld_shadow::GameWorldShadow;
        use crate::presentation_frame::presentation_from_gameworld_enabled;
        use gamelogic::world::PlayerId;
        use gamelogic::world::entities::TemplateRef;
        use glam::Vec3;

        let mut logic = GameLogic::new();
        let mut t = ThingTemplate::new("EnvSeedRanger466");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("EnvSeedRanger466".into(), t);
        let host_id = logic
            .create_object("EnvSeedRanger466", Team::USA, Vec3::new(8.0, 0.0, 9.0))
            .expect("create host object");

        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let gw_id = {
            let tmpl = TemplateRef {
                name: "EnvSeedGwOnly466".into(),
                source: None,
            };
            shadow.world_mut().spawn_entity(
                tmpl,
                Some(PlayerId::from_index(0)),
                gamelogic::world::entities::Transform::new([70.0, 0.0, 71.0], 0.1),
                40.0,
            )
        };
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(gw_id) {
            e.max_health = 40.0;
            e.team_ordinal = 0;
        }
        if let Some(e) = shadow
            .entity_for_host(host_id)
            .and_then(|eid| shadow.world_mut().world_mut().entity_mut(eid))
        {
            e.transform.position = [10.0, 0.0, 11.0].into();
            e.health = 90.0;
            e.max_health = 100.0;
        }

        let host_only = seed_presentation_env_frame_from_host_and_shadow(&mut logic, 0, None);
        assert!(
            host_only.objects.iter().any(|o| o.id == host_id),
            "host-only freeze must include host object"
        );
        let synth = 0x8000_0000 | gw_id.get();
        assert!(
            host_only.objects.iter().all(|o| o.id.0 != synth),
            "host-only freeze must not include GameWorld-only spawn"
        );

        let with_shadow =
            seed_presentation_env_frame_from_host_and_shadow(&mut logic, 0, Some(&shadow));
        let Some(host_obj) = with_shadow.objects.iter().find(|o| o.id == host_id) else {
            panic!("shadow freeze must keep host object");
        };
        assert!(
            (host_obj.position.x - 10.0).abs() < 1e-3
                && (host_obj.health_current - 90.0).abs() < 1e-3,
            "shadow freeze must overlay GameWorld pose/health: {:?}",
            host_obj.position
        );
        if presentation_from_gameworld_enabled() {
            assert!(
                with_shadow.objects.iter().any(|o| o.id.0 == synth),
                "shadow freeze must append GameWorld-only object"
            );
        }
    }

    #[test]
    fn seed_presentation_env_frame_matches_build_for_engine() {
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
        use crate::gameworld_shadow::GameWorldShadow;
        use crate::presentation_frame::PresentationFrame;
        use glam::Vec3;

        let mut logic = GameLogic::new();
        let mut t = ThingTemplate::new("EnvSeedEq466");
        t.set_health(80.0);
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("EnvSeedEq466".into(), t);
        let _ = logic
            .create_object("EnvSeedEq466", Team::USA, Vec3::new(3.0, 0.0, 4.0))
            .expect("create");
        let mut shadow = GameWorldShadow::new(32);
        shadow.sync_from_host(&logic);

        let via_helper =
            seed_presentation_env_frame_from_host_and_shadow(&mut logic, 0, Some(&shadow));
        let via_shipped = PresentationFrame::build_for_engine(&mut logic, 0, Some(&shadow));
        assert_eq!(
            via_helper.objects.len(),
            via_shipped.objects.len(),
            "env seed helper must be PresentationFrame::build_for_engine"
        );
        assert_eq!(
            via_helper
                .objects
                .iter()
                .map(|o| (o.id, o.position.x, o.health_current))
                .collect::<Vec<_>>(),
            via_shipped
                .objects
                .iter()
                .map(|o| (o.id, o.position.x, o.health_current))
                .collect::<Vec<_>>(),
        );
    }
}
