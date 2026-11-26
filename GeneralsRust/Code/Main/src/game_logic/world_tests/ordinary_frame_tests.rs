//! Timed object behaviors through the ordinary logic frame and presentation boundary.

use super::*;

mod terrain_decal_cadence;

fn owned_frame(logic: &mut GameLogic) {
    logic.tick_logic_frame(LOGIC_FRAME_TIMESTEP, None, None);
    logic.complete_presentation_observations();
}

#[test]
fn base_regeneration_self_repairs_and_publishes_health() {
    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("AmericaCommandCenter");
    tpl.add_kind_of(KindOf::Structure).set_health(1000.0);
    logic
        .templates
        .insert("AmericaCommandCenter".to_string(), tpl);
    let id = logic
        .create_object("AmericaCommandCenter", Team::USA, glam::Vec3::ZERO)
        .expect("cc");
    assert!(logic.host_object(id).unwrap().base_regenerate.is_some());

    {
        let o = logic.host_object_mut(id).unwrap();
        o.health.current = 500.0;
        o.status.under_construction = false;
    }
    logic.set_current_frame(0);
    logic.notify_base_regenerate_damage(id, false);

    // 95 logic frames cross the 90-frame damage-delay wake (heal rate: 3).
    for _ in 0..95 {
        owned_frame(&mut logic);
    }
    let hp = logic.host_object(id).unwrap().health.current;
    assert!(
        hp > 500.0,
        "structure must self-repair after the regeneration delay (hp={hp})"
    );
    assert!(logic.base_regenerate_reg.heal_ticks >= 1);
    let frame =
        crate::presentation_frame::PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    let structure = frame.objects.iter().find(|row| row.id == id).unwrap();
    assert_eq!(structure.health_current, hp);
}

#[test]
fn crate_lifetime_retires_on_its_deadline() {
    let mut logic = GameLogic::new();
    logic
        .admit_object_ini_sources([(
            "crate-host-tick.ini".into(),
            "Object SalvageCrate\nKindOf = CRATE\nBody = ActiveBody BodyTag\nMaxHealth = 50\nInitialHealth = 50\nEnd\nBehavior = DeletionUpdate ClockTag\nMinLifetime = 100\nMaxLifetime = 100\nEnd\nEnd\n".into(),
        )])
        .unwrap();
    let cid = logic
        .create_object("SalvageCrate", Team::Neutral, glam::Vec3::ZERO)
        .expect("crate");
    logic.host_money_crates.register_salvage_crate(cid, 40);

    for _ in 0..3 {
        owned_frame(&mut logic);
        assert!(logic.host_object(cid).is_some());
    }
    owned_frame(&mut logic);
    assert!(
        !logic.host_money_crates.contains(cid),
        "crate DeletionUpdate lifetime must expire during the logic update"
    );
    assert!(
        logic.objects.get(&cid).is_none(),
        "expired crate must be destroyed at its deadline"
    );
}

#[test]
fn fire_spreads_to_a_neighbor_during_logic_updates() {
    let mut logic = GameLogic::new();
    crate::game_logic::host_fire_spread::admit_fire_fixture(
        &mut logic,
        &["DogwoodTreeA", "DogwoodTreeB"],
        &[],
    );
    let a = logic
        .create_object("DogwoodTreeA", Team::Neutral, glam::Vec3::ZERO)
        .expect("tree a");
    let b = logic
        .create_object(
            "DogwoodTreeB",
            Team::Neutral,
            glam::Vec3::new(
                crate::game_logic::host_fire_spread::TREE_SPREAD_TRY_RANGE * 0.5,
                0.0,
                0.0,
            ),
        )
        .expect("tree b");
    assert!(logic.host_object(a).unwrap().has_fire_spread());
    assert!(logic.host_object(b).unwrap().has_fire_spread());

    assert!(logic.ignite_object_fire_spread(
        a,
        &mut crate::game_logic::ScriptPresentationEffects::default()
    ));
    {
        // Force the first spread attempt due immediately. It precedes the
        // partition phase inside the same tick (cells fill after), so pin a
        // short fixed reschedule and let the retry find the seeded cells.
        let o = logic.host_object_mut(a).unwrap();
        if let Some(fs) = o.fire_spread.as_mut() {
            fs.next_spread_frame = 0;
            fs.min_spread_delay = 5;
            fs.max_spread_delay = 5;
        }
    }

    // Cross frame 30 (the direct-call precedent's spread-due frame).
    for _ in 0..35 {
        owned_frame(&mut logic);
    }
    assert!(
        logic.fire_spread_reg.spreads > 0,
        "aflame tree must attempt spread during the logic update"
    );
    assert!(
        logic
            .host_object(b)
            .and_then(|o| o.fire_spread.as_ref().map(|f| f.is_aflame()))
            .unwrap_or(false)
            || logic.fire_spread_reg.ignitions >= 2,
        "neighbor within range must ignite during the logic update"
    );
}

#[test]
fn toxin_scud_field_damages_objects_after_the_blast() {
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("PoisonSponge");
    t.set_health(5000.0);
    t.add_kind_of(KindOf::Infantry);
    logic.templates.insert("PoisonSponge".into(), t);
    let inf = logic
        .create_object("PoisonSponge", Team::USA, glam::Vec3::ZERO)
        .expect("infantry");

    // Detonate a toxin SCUD warhead on the infantry: spawns the
    // MediumPoisonField DoT zone (host residual registry).
    let _ = logic
        .apply_scud_area_at(glam::Vec3::ZERO, None, Team::GLA, true)
        .unwrap();
    assert!(
        logic.scud_poison_zones().active_count() >= 1,
        "toxin blast must spawn the poison field zone"
    );
    let after_blast = logic.host_object(inf).unwrap().health.current;
    assert!(
        (after_blast - 5000.0).abs() > 0.01,
        "blast itself must land (hp={after_blast})"
    );

    // Zone ticks roughly every 15 frames after activation.
    for _ in 0..50 {
        owned_frame(&mut logic);
    }
    let after = logic.host_object(inf).unwrap().health.current;
    assert!(
        after < after_blast,
        "poison field DoT must tick at its deadline (blast={after_blast} after={after})"
    );
}
