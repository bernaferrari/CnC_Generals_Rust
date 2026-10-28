//! Host combat `impl GameLogic` — `ocl_and_scud`.
//! Child of `world_combat` (itself a child of `game_logic.rs`).
#![allow(unused_imports, non_snake_case)]
use super::super::*;

/// Results already created by an OCL stay committed if a later nugget fails.
/// Receipts retain the birth identity needed by reentrant callers.
pub(crate) struct OclExecutionOutcome {
    pub(crate) created: Vec<(ObjectId, std::sync::Weak<()>)>,
    pub(crate) error: Option<gamelogic::GameLogicError>,
}

impl OclExecutionOutcome {
    fn success(created: Vec<(ObjectId, std::sync::Weak<()>)>) -> Self {
        Self { created, error: None }
    }

    fn failure(
        created: Vec<(ObjectId, std::sync::Weak<()>)>,
        error: gamelogic::GameLogicError,
    ) -> Self {
        Self { created, error: Some(error) }
    }
}

/// Whether this host bridge can reproduce a parsed generic OCL nugget.
/// Unsupported physics/container/debris nuggets are skipped individually —
/// C++ `ObjectCreationList::createInternal` still runs every later nugget.
fn supports_host_weapon_generic_ocl(
    nugget: &gamelogic::object_creation_list::GenericObjectCreationNugget,
) -> bool {
    use gamelogic::object_creation_list::DebrisDisposition;

    let unsupported_disposition = DebrisDisposition::SEND_IT_FLYING
        | DebrisDisposition::SEND_IT_UP
        | DebrisDisposition::SEND_IT_OUT
        | DebrisDisposition::RANDOM_FORCE
        | DebrisDisposition::FLOATING
        | DebrisDisposition::WHIRLING;

    nugget.name_are_objects
        && !nugget.names.is_empty()
        && nugget.debris_to_generate > 0
        && !nugget.disposition.has(unsupported_disposition)
        && nugget.put_in_container.trim().is_empty()
        && !nugget.ignore_primary_obstacle
        && !nugget.contain_inside_source_object
        && !nugget.spread_formation
        && !nugget.dies_on_bad_land
        && !nugget.requires_live_player
}

#[cfg(test)]
mod receiving_debris_ocl_owner_tests {
    use super::*;

    fn world(
        model_names: &str,
        count: u32,
        mass: f32,
        min_lifetime: u32,
        max_lifetime: u32,
    ) -> (GameLogic, ObjectId) {
        let mut world = GameLogic::new();
        world
            .admit_object_creation_list_sources([format!(
                "ObjectCreationList OCL_ReceivingBridgeDebris\nCreateDebris\nModelNames = {model_names}\nCount = {count}\nMass = {mass}\nOffset = 1 2 3\nMinLifetime = {min_lifetime}f\nMaxLifetime = {max_lifetime}f\nDisposition = SEND_IT_FLYING\nDispositionIntensity = 1\nEnd\nEnd\n"
            )])
            .expect("OCL belongs to this session");
        let mut source_template = ThingTemplate::new("ReceivingBridgeSource");
        source_template
            .add_kind_of(crate::game_logic::KindOf::Structure)
            .set_health(100.0);
        let mut debris_template = ThingTemplate::new("GenericDebris");
        debris_template
            .add_kind_of(crate::game_logic::KindOf::Projectile)
            .set_health(100.0);
        debris_template.native_lifecycle_definitions = Some(
            vec![crate::game_logic::native_lifecycle_definition::NativeLifecycleDefinition {
                module_tag: Some("DebrisLife".into()),
                kind: crate::game_logic::native_lifecycle_definition::NativeLifecycleKind::Lifetime(
                    crate::game_logic::native_lifecycle_definition::LifetimeFrameRange {
                        minimum: 1,
                        maximum: 1,
                    },
                ),
            }]
            .into_boxed_slice(),
        );
        world
            .templates
            .insert("ReceivingBridgeSource".into(), source_template);
        world
            .templates
            .insert("GenericDebris".into(), debris_template);
        // The receiving model override is only meaningful through an authored
        // W3DDebrisDraw receiver. Keep this fixture on the same module path as
        // the retail GenericDebris template rather than inventing a Draw row.
        let mut parser = crate::assets::IniParser::new();
        parser
            .parse_ini_content(
                "Object GenericDebris\nKindOf = PROJECTILE\nBody = ActiveBody DebrisBody\nMaxHealth = 100\nInitialHealth = 100\nEnd\nDraw = W3DDebrisDraw DebrisDrawTag\nEnd\nEnd\n",
                "generic-debris-owner-test.ini",
            )
            .expect("authored GenericDebris Draw fixture");
        let definition = parser
            .get_definition("GenericDebris")
            .expect("GenericDebris definition")
            .clone();
        world.seed_asset_definition_templates_from_snapshot([("GenericDebris".into(), definition)]);
        let source = world
            .create_object(
                "ReceivingBridgeSource",
                Team::USA,
                Vec3::new(20.0, 0.0, 30.0),
            )
            .expect("source object");
        world
            .objects
            .get_mut(&source)
            .expect("source Object")
            .set_orientation_with_partition_owner(
                std::f32::consts::FRAC_PI_2,
                &mut world.partition_manager,
            );
        (world, source)
    }

    #[test]
    fn later_factory_error_retains_the_ordered_committed_ocl_prefix() {
        let mut world = GameLogic::new();
        world
            .admit_object_creation_list_sources(["ObjectCreationList OCL_PartialFactoryError\nCreateObject\nObjectNames = OclPartialGood\nCount = 1\nEnd\nCreateObject\nObjectNames = OclPartialBad\nCount = 1\nEnd\nEnd\n".into()])
            .expect("OCL is valid receiving content");
        let mut source = ThingTemplate::new("OclPartialSource");
        source.add_kind_of(crate::game_logic::KindOf::Structure).set_health(100.0);
        let mut good = ThingTemplate::new("OclPartialGood");
        good.add_kind_of(crate::game_logic::KindOf::Vehicle).set_health(100.0);
        let mut bad = ThingTemplate::new("OclPartialBad");
        bad.add_kind_of(crate::game_logic::KindOf::Vehicle).set_health(100.0);
        // Private factory fault injection after receiving OCL admission. The
        // public content parser correctly rejects this malformed Body field;
        // this test targets the later-factory-error boundary itself.
        bad.body_definition_error = Some("injected checked factory error".into());
        world.templates.insert(source.name.clone(), source);
        world.templates.insert(good.name.clone(), good);
        world.templates.insert(bad.name.clone(), bad);
        let source_id = world
            .create_object("OclPartialSource", Team::USA, Vec3::ZERO)
            .expect("OCL source object");

        let created = world.execute_parsed_weapon_ocl_at(
            "OCL_PartialFactoryError",
            Some(source_id),
            Team::USA,
            VeterancyLevel::Rookie,
            0.0,
            Vec3::ZERO,
            Vec3::ZERO,
        );

        assert_eq!(created.len(), 1);
        assert_eq!(world.host_object(created[0]).unwrap().template_name, "OclPartialGood");
        assert!(world
            .host_objects()
            .registered_values()
            .all(|object| object.template_name != "OclPartialBad"));
    }

    #[test]
    fn parsed_bridge_debris_is_owned_per_world_and_uses_receiving_lod_mask() {
        let (mut left, left_source) = world("LeftChunk", 2, 5.0, 10, 20);
        let (mut right, right_source) = world("RightChunk", 2, 5.0, 10, 20);
        assert_eq!(left_source, right_source);

        // C++ isDebrisSkipped increments then masks; mask 1 keeps generation 1
        // and skips generation 2. The peer world has an independent counter.
        left.admit_dynamic_debris_skip_mask(1);
        let left_ids = left.execute_parsed_weapon_ocl_at(
            "OCL_ReceivingBridgeDebris",
            Some(left_source),
            Team::USA,
            VeterancyLevel::Rookie,
            0.0,
            Vec3::ZERO,
            Vec3::new(20.0, 0.0, 30.0),
        );
        let right_ids = right.execute_parsed_weapon_ocl_at(
            "OCL_ReceivingBridgeDebris",
            Some(right_source),
            Team::USA,
            VeterancyLevel::Rookie,
            0.0,
            Vec3::ZERO,
            Vec3::new(20.0, 0.0, 30.0),
        );

        assert_eq!(left_ids.len(), 1);
        assert_eq!(right_ids.len(), 2);
        assert_eq!(left_ids[0], right_ids[0]);
        let left_chunk = left.objects.get(&left_ids[0]).expect("left debris");
        let right_chunk = right.objects.get(&right_ids[0]).expect("right debris");
        assert_eq!(left_chunk.template_name, "GenericDebris");
        assert_eq!(right_chunk.template_name, "GenericDebris");
        assert_eq!(
            left_chunk.debris_draw.as_ref().unwrap().model_name,
            "LeftChunk"
        );
        assert_eq!(
            right_chunk.debris_draw.as_ref().unwrap().model_name,
            "RightChunk"
        );
        assert_eq!(
            left_chunk.get_visual_info().model_name.as_deref(),
            Some("LeftChunk")
        );
        assert_eq!(
            right_chunk.get_visual_info().model_name.as_deref(),
            Some("RightChunk")
        );
        assert_eq!(left_chunk.physics_mass, 5.0);
        assert_eq!(right_chunk.physics_mass, 5.0);
        let expected_offset =
            glam::Mat3::from_rotation_z(std::f32::consts::FRAC_PI_2) * Vec3::new(1.0, 2.0, 3.0);
        assert!(
            (left_chunk.get_position() - (Vec3::new(20.0, 0.0, 30.0) + expected_offset)).length()
                < 1e-4
        );
        assert_eq!(
            left.live_drawable_model_names(),
            vec!["LeftChunk".to_owned()]
        );
        assert_eq!(
            right.live_drawable_model_names(),
            vec!["RightChunk".to_owned()]
        );
        assert_eq!(left.templates["GenericDebris"].model_name, None);
        assert_eq!(right.templates["GenericDebris"].model_name, None);

        // Exercise the production presentation projection consumed by
        // RenderPipeline, not only ObjectVisualInfo or the shell preloader.
        let left_frame =
            crate::presentation_frame::PresentationFrame::build_from_logic(&mut left, 0);
        let right_frame =
            crate::presentation_frame::PresentationFrame::build_from_logic(&mut right, 0);
        let left_mesh = left_frame
            .unit_render_inputs()
            .into_iter()
            .find(|input| input.id == left_ids[0])
            .expect("left debris reaches the production mesh collector");
        let right_mesh = right_frame
            .unit_render_inputs()
            .into_iter()
            .find(|input| input.id == right_ids[0])
            .expect("right debris reaches the production mesh collector");
        assert_eq!(left_mesh.template_name, "GenericDebris");
        assert_eq!(right_mesh.template_name, "GenericDebris");
        assert_eq!(
            left_mesh
                .receiving_draw_models
                .as_deref()
                .unwrap()
                .module_class_names()
                .as_slice(),
            ["W3DDebrisDraw"]
        );
        assert_eq!(
            right_mesh
                .receiving_draw_models
                .as_deref()
                .unwrap()
                .module_class_names()
                .as_slice(),
            ["W3DDebrisDraw"]
        );
        assert_eq!(left_mesh.model_key, "LeftChunk");
        assert_eq!(right_mesh.model_key, "RightChunk");
        assert_eq!(left_mesh.draw_models[0].module_index, 0);
        assert_eq!(right_mesh.draw_models[0].module_index, 0);
        assert_eq!(left_mesh.draw_models[0].model_key, "LeftChunk");
        assert_eq!(right_mesh.draw_models[0].model_key, "RightChunk");
    }

    #[test]
    fn debris_model_override_does_not_invent_missing_receiving_draw_geometry() {
        let mut world = GameLogic::new();
        world
            .admit_object_creation_list_sources(["ObjectCreationList OCL_NoDebrisDraw\nCreateDebris\nModelNames = AuthoredChunk\nCount = 1\nMass = 5\nDisposition = SEND_IT_FLYING\nDispositionIntensity = 1\nEnd\nEnd".into()])
            .unwrap();
        let mut source_template = ThingTemplate::new("NoDrawSource");
        source_template.set_health(100.0);
        let mut debris_template = ThingTemplate::new("GenericDebris");
        debris_template
            .add_kind_of(crate::game_logic::KindOf::Projectile)
            .set_health(100.0);
        world
            .templates
            .insert("NoDrawSource".into(), source_template);
        world
            .templates
            .insert("GenericDebris".into(), debris_template);
        let source = world
            .create_object("NoDrawSource", Team::USA, Vec3::ZERO)
            .unwrap();
        let ids = world.execute_parsed_weapon_ocl_at(
            "OCL_NoDebrisDraw",
            Some(source),
            Team::USA,
            VeterancyLevel::Rookie,
            0.0,
            Vec3::ZERO,
            Vec3::ZERO,
        );
        assert_eq!(ids.len(), 1);
        assert_eq!(
            world.objects[&ids[0]]
                .debris_draw
                .as_ref()
                .unwrap()
                .model_name,
            "AuthoredChunk"
        );

        let frame = crate::presentation_frame::PresentationFrame::build_from_logic(&mut world, 0);
        let input = frame
            .unit_render_inputs()
            .into_iter()
            .find(|input| input.id == ids[0])
            .unwrap();
        assert_eq!(input.template_name, "GenericDebris");
        assert!(input.receiving_draw_models.is_none());
        assert_ne!(input.model_key, "AuthoredChunk");
        assert!(
            input
                .draw_models
                .iter()
                .all(|draw| draw.model_key != "AuthoredChunk")
        );
    }

    #[test]
    fn authored_debris_model_choices_mass_and_lifetime_ranges_stay_with_each_owner() {
        let (mut left, left_source) = world("LeftA LeftB", 6, 5.0, 10, 20);
        let (mut right, right_source) = world("RightA RightB", 6, 9.0, 30, 40);
        assert_eq!(left_source, right_source);

        let left_ids = left.execute_parsed_weapon_ocl_at(
            "OCL_ReceivingBridgeDebris",
            Some(left_source),
            Team::USA,
            VeterancyLevel::Rookie,
            0.0,
            Vec3::ZERO,
            Vec3::new(20.0, 0.0, 30.0),
        );
        let right_ids = right.execute_parsed_weapon_ocl_at(
            "OCL_ReceivingBridgeDebris",
            Some(right_source),
            Team::USA,
            VeterancyLevel::Rookie,
            0.0,
            Vec3::ZERO,
            Vec3::new(20.0, 0.0, 30.0),
        );
        assert_eq!(left_ids.len(), 6);
        assert_eq!(right_ids.len(), 6);

        let mut left_choices = Vec::new();
        let mut right_choices = Vec::new();
        for (left_id, right_id) in left_ids.iter().zip(&right_ids) {
            let left_debris = left.objects.get(left_id).expect("left debris object");
            let right_debris = right.objects.get(right_id).expect("right debris object");
            assert_eq!(left_debris.template_name, "GenericDebris");
            assert_eq!(right_debris.template_name, "GenericDebris");
            let left_model = left_debris
                .debris_draw
                .as_ref()
                .unwrap()
                .model_name
                .as_str();
            let right_model = right_debris
                .debris_draw
                .as_ref()
                .unwrap()
                .model_name
                .as_str();
            assert!(matches!(left_model, "LeftA" | "LeftB"));
            assert!(matches!(right_model, "RightA" | "RightB"));
            // OCL ModelNames performs one receiving logic-RNG choice per item;
            // peer owners start from the same seed, so the selected list index
            // is equal despite distinct model namespaces and lifetime ranges.
            left_choices.push(left_model.ends_with('B'));
            right_choices.push(right_model.ends_with('B'));
            assert_eq!(left_debris.physics_mass, 5.0);
            assert_eq!(right_debris.physics_mass, 9.0);
            let left_deadline = left_debris
                .native_lifecycle
                .as_ref()
                .and_then(|runtime| runtime.first_lifetime_deadline())
                .expect("receiving LifetimeUpdate");
            let right_deadline = right_debris
                .native_lifecycle
                .as_ref()
                .and_then(|runtime| runtime.first_lifetime_deadline())
                .expect("receiving LifetimeUpdate");
            assert!((10..=20).contains(&left_deadline.saturating_sub(left.frame)));
            assert!((30..=40).contains(&right_deadline.saturating_sub(right.frame)));
        }
        assert_eq!(left_choices, right_choices);

        let left_frame =
            crate::presentation_frame::PresentationFrame::build_from_logic(&mut left, 0);
        let right_frame =
            crate::presentation_frame::PresentationFrame::build_from_logic(&mut right, 0);
        for (id, model) in left_ids.iter().zip(left_choices.iter()) {
            let expected = if *model { "LeftB" } else { "LeftA" };
            let submitted = left_frame
                .unit_render_inputs()
                .into_iter()
                .find(|input| input.id == *id)
                .unwrap();
            assert_eq!(submitted.model_key, expected);
        }
        for (id, model) in right_ids.iter().zip(right_choices.iter()) {
            let expected = if *model { "RightB" } else { "RightA" };
            let submitted = right_frame
                .unit_render_inputs()
                .into_iter()
                .find(|input| input.id == *id)
                .unwrap();
            assert_eq!(submitted.model_key, expected);
        }
    }
}

fn unsupported_host_ocl_nugget_name(
    nugget: &dyn gamelogic::object_creation_list::ObjectCreationNugget,
) -> &'static str {
    let nugget = nugget.as_any();
    use gamelogic::object_creation_list::advanced_nuggets::{
        ApplyRandomForceNugget, AttackNugget, DeliverPayloadNugget, FireWeaponNugget,
    };
    if nugget.is::<FireWeaponNugget>() {
        "FireWeapon"
    } else if nugget.is::<DeliverPayloadNugget>() {
        "DeliverPayload"
    } else if nugget.is::<AttackNugget>() {
        "Attack"
    } else if nugget.is::<ApplyRandomForceNugget>() {
        "ApplyRandomForce"
    } else {
        "other"
    }
}

fn host_ocl_disposition_bits(
    disposition: gamelogic::object_creation_list::DebrisDisposition,
) -> u32 {
    use gamelogic::object_creation_list::DebrisDisposition as D;
    [
        D::LIKE_EXISTING,
        D::ON_GROUND_ALIGNED,
        D::SEND_IT_FLYING,
        D::SEND_IT_UP,
        D::SEND_IT_OUT,
        D::RANDOM_FORCE,
        D::FLOATING,
        D::INHERIT_VELOCITY,
        D::WHIRLING,
    ]
    .into_iter()
    .filter(|flag| disposition.has(*flag))
    .fold(0, |bits, flag| bits | flag)
}

fn usable_ocl_particle_sys_name(name: &str) -> Option<&str> {
    let name = name.trim();
    (!name.is_empty() && !name.eq_ignore_ascii_case("none")).then_some(name)
}

/// C++ `getPrimaryDamageRadius` residual for FireFX / DetonationFX overrideRadius.
fn host_fire_fx_override_radius(shooter: Option<&Object>) -> f32 {
    shooter
        .and_then(|o| o.weapon_name_for_slot(0))
        .map(crate::game_logic::weapon_bootstrap::host_primary_damage_radius_for_weapon_name)
        .unwrap_or(0.0)
}

impl GameLogic {
    /// Execute a parsed `Weapon.ini` OCL at a source origin.
    ///
    /// C++ `Weapon::fireWeaponTemplate` calls `ObjectCreationList::create`
    /// with the firing object for `FireOCL`, and with the live missile object
    /// for `ProjectileDetonationOCL` (`Weapon.cpp:943-949`,
    /// `MissileAIUpdate.cpp:364-381`).  The host has no simulation `Object`
    /// for every visual projectile, so the caller supplies that object's
    /// frozen team/rank and its fire/impact origin.  References are resolved
    /// only through the receiving session's admitted OCL and object content;
    /// unresolved lists and templates do not select another world's assets.
    /// Object creation runs synchronously against this instance's templates,
    /// RNG, and object ownership.
    pub fn execute_parsed_weapon_ocl_at(
        &mut self,
        ocl_name: &str,
        source_id: Option<ObjectId>,
        source_team: Team,
        source_veterancy: VeterancyLevel,
        source_orientation: f32,
        source_velocity: Vec3,
        origin: Vec3,
    ) -> Vec<ObjectId> {
        self.execute_parsed_weapon_ocl_at_with_effects(
            ocl_name,
            source_id,
            source_team,
            source_veterancy,
            source_orientation,
            source_velocity,
            origin,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn execute_parsed_weapon_ocl_at_with_effects(
        &mut self,
        ocl_name: &str,
        source_id: Option<ObjectId>,
        source_team: Team,
        source_veterancy: VeterancyLevel,
        source_orientation: f32,
        source_velocity: Vec3,
        origin: Vec3,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Vec<ObjectId> {
        self.execute_parsed_weapon_ocl_at_with_interpreter_and_effects(
            ocl_name,
            source_id,
            source_team,
            source_veterancy,
            source_orientation,
            source_velocity,
            origin,
            None,
            presentation_effects,
        )
    }

    pub(crate) fn execute_parsed_weapon_ocl_at_with_interpreter_and_effects(
        &mut self,
        ocl_name: &str,
        source_id: Option<ObjectId>,
        source_team: Team,
        source_veterancy: VeterancyLevel,
        source_orientation: f32,
        source_velocity: Vec3,
        origin: Vec3,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Vec<ObjectId> {
        let outcome = self.execute_parsed_weapon_ocl_with_births(
            ocl_name,
            source_id,
            source_team,
            source_veterancy,
            source_orientation,
            source_velocity,
            origin,
            interpreter,
            presentation_effects,
        );
        if let Some(error) = outcome.error {
            log::error!("{error}");
        }
        outcome
            .created
            .into_iter()
            .filter_map(|(id, birth)| self.object_birth_matches(id, &birth).then_some(id))
            .collect()
    }

    pub(crate) fn execute_parsed_weapon_ocl_with_births(
        &mut self,
        ocl_name: &str,
        source_id: Option<ObjectId>,
        source_team: Team,
        source_veterancy: VeterancyLevel,
        source_orientation: f32,
        source_velocity: Vec3,
        origin: Vec3,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> OclExecutionOutcome {
        use gamelogic::object_creation_list::{
            DebrisDisposition, GenericObjectCreationNugget, ObjectCreationNugget,
        };

        let ocl_name = ocl_name.trim();
        if source_id.is_none() || ocl_name.is_empty() || ocl_name.eq_ignore_ascii_case("None") {
            return OclExecutionOutcome::success(Vec::new());
        }

        let Some(ocl) = self.object_creation_list_definition(ocl_name) else {
            log::warn!(
                "receiving OCL `{ocl_name}` was not admitted for this GameLogic; no global fallback was used"
            );
            return OclExecutionOutcome::success(Vec::new());
        };

        // C++ ObjectCreationList.cpp:1524-1534 createInternal walks every nugget
        // independently. A leftover-physics CreateDebris/FireWeapon/etc. must not
        // discard earlier CreateObject results from the same authored list.
        let mut created = Vec::new();
        for nugget in ocl.nuggets() {
            let Some(generic) = nugget
                .as_any()
                .downcast_ref::<GenericObjectCreationNugget>()
            else {
                log::warn!(
                    "receiving OCL `{ocl_name}` skipped unsupported {} nugget; no Core OCL fallback was used",
                    unsupported_host_ocl_nugget_name(nugget.as_ref())
                );
                continue;
            };
            if generic.debris_to_generate == 0 {
                continue;
            }
            if !generic.name_are_objects {
                let debris = self.execute_receiving_create_debris(
                    generic,
                    source_id.expect("validated above"),
                    source_team,
                    origin,
                    presentation_effects,
                );
                created.extend(debris.created);
                if let Some(error) = debris.error {
                    return OclExecutionOutcome::failure(created, error);
                }
                continue;
            }
            if !supports_host_weapon_generic_ocl(generic) {
                log::warn!(
                    "receiving OCL `{ocl_name}` skipped unsupported GenericObjectCreationNugget configuration"
                );
                continue;
            }

            // `GenericObjectCreationNugget::create` checks source altitude
            // before spawning.  The host coordinate system is Y-up, whereas
            // C++ Coord3D is Z-up; terrain_height_at already accepts host X/Z.
            if generic.skip_if_significantly_airborne {
                let terrain_y = self.terrain_height_at(origin).unwrap_or(origin.y);
                if crate::game_logic::host_usa_pilot::is_significantly_above_terrain(
                    origin.y - terrain_y,
                ) {
                    continue;
                }
            }

            let count = usize::try_from(generic.debris_to_generate).unwrap_or_default();
            let upper_pick = match i32::try_from(generic.names.len().saturating_sub(1)) {
                Ok(value) => value,
                Err(_) => continue,
            };
            for _ in 0..count {
                // C++ chooses once per created object from ObjectNames using
                // the game-logic random stream (ObjectCreationList.cpp:1326).
                let raw_pick = self.logic_random.next_int(0, upper_pick);
                let Ok(pick) = usize::try_from(raw_pick) else {
                    continue;
                };
                let Some(template_name) = generic.names.get(pick) else {
                    continue;
                };
                let template_name = template_name.trim();
                if template_name.is_empty() || template_name.eq_ignore_ascii_case("None") {
                    continue;
                }

                // The receiving session admitted its authored templates before
                // the match. A missing name cannot be recovered from another
                // application's asset manager or a decorative fallback.
                if !self.templates.contains_key(template_name) {
                    log::warn!(
                        "receiving OCL `{ocl_name}` references unadmitted object template `{template_name}`"
                    );
                    continue;
                }

                // C++ `GenericObjectCreationNugget::doStuffToObj` transforms
                // `Offset` through the primary object's matrix before adding
                // it to the source position (ObjectCreationList.cpp:953-960).
                // `Coord3D` is (X, Y-horizontal, Z-up), whereas the host is
                // X/Z-horizontal with Y-up, so convert first and then apply
                // the frozen source yaw.  Leaving this unrotated puts every
                // directional FireOCL/debris spawn on the same world side of
                // its source regardless of facing.
                let local_offset = Vec3::new(generic.offset.x, generic.offset.z, generic.offset.y);
                let rotated_offset = glam::Mat3::from_rotation_y(source_orientation) * local_offset;
                let mut spawn_position = origin + rotated_offset;
                let on_ground = generic
                    .disposition
                    .has(DebrisDisposition::ON_GROUND_ALIGNED);
                if on_ground {
                    spawn_position.y = self
                        .terrain_height_at(spawn_position)
                        .unwrap_or(spawn_position.y);
                }

                // C++ ObjectCreationList.cpp:1302-1305
                // `sourceObj->getControllingPlayer()->getDefaultTeam()`.
                // `create_object` uses `unique_player_id_for_team`, which is
                // None in 2v2 same-faction, dropping the controlling player.
                let source_owner_player_id =
                    source_id.and_then(|id| self.objects.get(&id).and_then(|o| o.owner_player_id));
                let creation = self
                    .create_object_for_owner_or_team_with_birth_checked(
                        template_name,
                        source_team,
                        source_owner_player_id,
                        spawn_position,
                        interpreter,
                        presentation_effects,
                    );
                let (created_id, created_birth) = match creation {
                    Ok(Some(created)) => created,
                    Ok(None) => continue,
                    Err(error) => {
                        if let Some(committed) = error.committed {
                            created.push(committed);
                        }
                        return OclExecutionOutcome::failure(created, error.error);
                    }
                };
                // Registration is already committed, even if a later callback
                // removes this birth before the OCL finishes.
                created.push((created_id, std::sync::Weak::clone(&created_birth)));
                let birth_matches = |world: &Self| {
                    world.objects.get(&created_id).is_some_and(|object| {
                        created_birth.ptr_eq(&std::sync::Arc::downgrade(
                            &object.visual_draw_identity_token,
                        ))
                    })
                };
                if !birth_matches(self) {
                    continue;
                }
                if let Some(object) = self.objects.get_mut(&created_id) {
                    object.producer_id = source_id;
                    if generic.disposition.has(DebrisDisposition::LIKE_EXISTING) {
                        object.set_orientation_with_partition_owner(
                            source_orientation,
                            &mut self.partition_manager,
                        );
                    }
                }
                let veterancy_health_bonus = self.veterancy_health_bonus;
                let inherit_name = self
                    .with_object_veterancy_feedback_and_locomotors(
                        created_id,
                        |object, audio, feedback, locomotors| {
                            if generic.disposition.has(DebrisDisposition::INHERIT_VELOCITY) {
                                // The host's movement/physics state is the concrete
                                // equivalent of C++ PhysicsBehavior::applyForce for this
                                // disposition.  It also preserves DragonTank FireWall
                                // segment direction without inspecting an OCL name.
                                object.movement.velocity = source_velocity;
                            }
                            // C++ ObjectCreationList.cpp:996-1006 — inherit rank and
                            // transferObjectName only when the created tracker isTrainable.
                            let inherit_name = generic.inherit_veterancy && object.is_trainable();
                            if inherit_name {
                                object.set_min_veterancy_level_with_health_bonus_table(
                                    source_veterancy,
                                    &veterancy_health_bonus,
                                    audio,
                                    feedback,
                                    locomotors,
                                );
                            }
                            inherit_name
                        },
                    )
                    .unwrap_or(false);
                if !birth_matches(self) {
                    continue;
                }
                let Some(object) = self.objects.get_mut(&created_id) else {
                    continue;
                };
                if generic.max_frames > 0 && object.native_lifecycle.is_some() {
                    object.set_native_lifetime_range(
                        self.frame,
                        crate::game_logic::native_lifecycle_definition::LifetimeFrameRange {
                            minimum: generic.min_frames,
                            maximum: generic.max_frames,
                        },
                        &self.logic_random,
                    );
                } else if generic.max_frames > 0 {
                    let min_frames = i32::try_from(generic.min_frames).unwrap_or(i32::MAX);
                    let max_frames = i32::try_from(generic.max_frames).unwrap_or(i32::MAX);
                    let chosen_frames = self.logic_random.next_int(min_frames, max_frames);
                    let delay_frames = u32::try_from(chosen_frames).unwrap_or(generic.min_frames);
                    object.lifetime_update = Some(
                        crate::game_logic::host_lifetime_update::HostLifetimeUpdateData::from_delay_frames(
                            self.frame,
                            delay_frames,
                        ),
                    );
                }

                // C++ calls BodyModuleInterface::setInitialHealth with a
                // game-logic random percent after the object is created.
                let health_percent = self
                    .logic_random
                    .next_real(generic.min_health, generic.max_health)
                    .clamp(0.0, 1.0);
                let target_health = object.body_initial_health() * health_percent;
                object.internal_change_health(target_health - object.health.current);

                if generic.invulnerable_time > 0 {
                    object.apply_eject_invulnerable_at(
                        self.frame.saturating_add(generic.invulnerable_time),
                        self.frame,
                    );
                }
                if on_ground {
                    // `ON_GROUND_ALIGNED` gives every spawned object a random
                    // yaw after terrain placement (ObjectCreationList.cpp).
                    object.set_orientation_with_partition_owner(
                        self.logic_random.next_real(0.0, std::f32::consts::TAU),
                        &mut self.partition_manager,
                    );
                }
                // C++ ObjectCreationList.cpp:1086/1121-1124 ExtraFriction/Bounciness/BounceSound.
                object.set_extra_friction(generic.extra_friction);
                if generic.disposition.has(
                    gamelogic::object_creation_list::DebrisDisposition::SEND_IT_FLYING
                        | gamelogic::object_creation_list::DebrisDisposition::SEND_IT_UP
                        | gamelogic::object_creation_list::DebrisDisposition::RANDOM_FORCE,
                ) {
                    object.set_extra_bounciness(generic.extra_bounciness);
                    object.set_allow_bouncing(true);
                }
                if !generic.bounce_sound.is_empty() {
                    object.set_bounce_sound(generic.bounce_sound.clone());
                }
                if generic.fade_in {
                    object.start_drawable_fade_in(generic.fade_frames, self.frame);
                } else if generic.fade_out {
                    object.start_drawable_fade_out(generic.fade_frames, self.frame);
                }
                drop(object);
                if inherit_name {
                    if let Some(sid) = source_id {
                        // C++ ObjectCreationList.cpp:1005
                        // TheScriptEngine->transferObjectName(sourceObj->getName(), obj)
                        let _ = self.transfer_script_object_name(sid, created_id);
                    }
                }
                if !birth_matches(self) {
                    continue;
                }
                // C++ ObjectCreationList.cpp:962-969 — ParticleSystem is extra
                // attached fire/smoke. The object still spawns.
                if let Some(particle) = usable_ocl_particle_sys_name(&generic.particle_sys_name) {
                    let is_shrouded = self.host_fx_object_is_shrouded(created_id.0);
                    let _ = self
                        .combat_particles
                        .attach_named_to_object_with_visibility(
                            created_id,
                            spawn_position,
                            self.frame,
                            particle,
                            is_shrouded,
                            presentation_effects,
                        );
                }
            }
        }
        OclExecutionOutcome::success(created)
    }

    /// Bounded receiving-session implementation of authored CreateDebris.
    /// This supports the live bridge transition slice (a required receiving
    /// GenericDebris template, per-item model-name selection, positive mass,
    /// optional receiving LifetimeUpdate range, and SEND_IT_FLYING disposition).
    /// Unsupported draw/physics options fail closed instead of substituting a
    /// model template or fallback geometry.
    fn execute_receiving_create_debris(
        &mut self,
        generic: &gamelogic::object_creation_list::GenericObjectCreationNugget,
        source_id: ObjectId,
        source_team: Team,
        origin: Vec3,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> OclExecutionOutcome {
        use gamelogic::object_creation_list::DebrisDisposition as D;
        use gamelogic::object_creation_list::StaticGameLODLevel;

        let supported = generic.debris_to_generate > 0
            && !generic.names.is_empty()
            && generic.mass > 0.0
            && generic.disposition.has(D::SEND_IT_FLYING)
            && !generic.disposition.has(
                D::LIKE_EXISTING
                    | D::ON_GROUND_ALIGNED
                    | D::SEND_IT_UP
                    | D::SEND_IT_OUT
                    | D::RANDOM_FORCE
                    | D::FLOATING
                    | D::INHERIT_VELOCITY
                    | D::WHIRLING,
            )
            && generic.put_in_container.trim().is_empty()
            && generic.particle_sys_name.trim().is_empty()
            && generic.anim_sets.is_empty()
            && generic.fx_final.is_none()
            && generic.min_lod_required == StaticGameLODLevel::Low
            && (generic.max_frames == 0 || generic.min_frames <= generic.max_frames)
            && !generic.fade_in
            && !generic.fade_out
            && !generic.spread_formation
            && !generic.contain_inside_source_object
            && !generic.ignore_primary_obstacle
            && !generic.skip_if_significantly_airborne
            && !generic.requires_live_player
            && !generic.dies_on_bad_land
            && !generic.orient_in_force_direction
            && generic.preserve_layer;
        if !supported {
            log::warn!(
                "receiving CreateDebris nugget is outside the GenericDebris bridge slice; no Core ThingFactory/GameLOD fallback was used"
            );
            return OclExecutionOutcome::success(Vec::new());
        }
        if !self.templates.contains_key("GenericDebris") {
            log::warn!(
                "receiving CreateDebris requires the admitted GenericDebris template; no fallback object geometry was created"
            );
            return OclExecutionOutcome::success(Vec::new());
        }
        let Some((owner_player_id, source_layer, source_matrix, source_orientation)) =
            self.objects.get(&source_id).map(|source| {
                (
                    source.owner_player_id,
                    source.pathfind_layer,
                    source.get_transform_matrix(),
                    source.get_orientation(),
                )
            })
        else {
            return OclExecutionOutcome::success(Vec::new());
        };
        let team = if owner_player_id.is_some() {
            source_team
        } else {
            Team::Neutral
        };
        self.ocl_create_debris_reg
            .record_plan(host_ocl_disposition_bits(generic.disposition));

        let mut created = Vec::new();
        let count = u32::try_from(generic.debris_to_generate).unwrap_or_default();
        for _ in 0..count {
            // C++ selects ModelNames before calling GameLODManager::isDebrisSkipped.
            let upper = i32::try_from(generic.names.len().saturating_sub(1)).unwrap_or(i32::MAX);
            let pick = usize::try_from(self.logic_random.next_int(0, upper))
                .unwrap_or_default()
                .min(generic.names.len() - 1);
            if self.ocl_create_debris_reg.is_debris_skipped() {
                continue;
            }
            let model_name = generic.names[pick].trim();
            if model_name.is_empty() || model_name.eq_ignore_ascii_case("none") {
                log::warn!("receiving CreateDebris selected an empty model name; item skipped");
                continue;
            }
            // C++ ObjectCreationList.cpp:953-960 transforms its Z-up Coord3D
            // Offset through the primary Object matrix before translating.
            // Thing's receiving transform matrix is in that same source basis;
            // transform the vector without applying the matrix translation.
            let local_offset = Vec3::new(generic.offset.x, generic.offset.y, generic.offset.z);
            let position = origin + source_matrix.transform_vector3(local_offset);
            let creation = self.create_object_for_owner_or_team_with_birth_checked(
                "GenericDebris",
                team,
                owner_player_id,
                position,
                None,
                presentation_effects,
            );
            let (id, birth) = match creation {
                Ok(Some(created)) => created,
                Ok(None) => continue,
                Err(error) => {
                    if let Some(committed) = error.committed {
                        created.push(committed);
                    }
                    return OclExecutionOutcome::failure(created, error.error);
                }
            };
            created.push((id, std::sync::Weak::clone(&birth)));
            if !self.objects.get(&id).is_some_and(|object| {
                birth.ptr_eq(&std::sync::Arc::downgrade(
                    &object.visual_draw_identity_token,
                ))
            }) {
                continue;
            }

            // C++ doStuffToObj sets producer and per-instance DebrisDraw state.
            // Keep model selection on the new Object; the shared template remains
            // unchanged for this and all other GameLogic owners.
            if let Some(object) = self.objects.get_mut(&id) {
                object.producer_id = Some(source_id);
                if generic.preserve_layer && source_layer != 1 {
                    object.pathfind_layer = source_layer;
                }
                // C++ doStuffToObj: override the first existing LifetimeUpdate
                // only when the nugget authored a positive maximum. The setter
                // consumes this receiving world's RNG after factory creation.
                if generic.max_frames > 0 {
                    let _ = object.set_native_lifetime_range(
                        self.frame,
                        crate::game_logic::native_lifecycle_definition::LifetimeFrameRange {
                            minimum: generic.min_frames,
                            maximum: generic.max_frames,
                        },
                        &self.logic_random,
                    );
                }
                object.debris_draw = Some(crate::game_logic::object::visual::DebrisDrawState {
                    model_name: model_name.to_owned(),
                    anim_initial: String::new(),
                    anim_flying: String::new(),
                    anim_final: String::new(),
                    fx_final: None,
                    allow_model_color_change: generic.ok_to_change_model_color,
                    shadow_type: generic.shadow_type,
                });

                // C++ calls setInitialHealth before processing dispositions.
                // It consumes a random health percentage even when no body is
                // present, so retain the draw before the flight randoms.
                let health_percent = self
                    .logic_random
                    .next_real(generic.min_health, generic.max_health)
                    .clamp(0.0, 1.0);
                let target_health = object.body_initial_health() * health_percent;
                object.internal_change_health(target_health - object.health.current);

                // C++ SEND_IT_FLYING force and angular-rate draw order.
                // Mass and bounce properties are installed after initial
                // health, at the start of disposition handling.
                object.physics_mass = generic.mass;
                object.extra_bounciness = generic.extra_bounciness;
                object.extra_friction = generic.extra_friction;
                object.set_allow_bouncing(true);
                object.bounce_sound_name = generic.bounce_sound.clone();
                let spin = if generic.spin_rate >= 0.0 {
                    generic.spin_rate
                } else {
                    (std::f32::consts::PI / 32.0) * generic.disposition_intensity
                };
                let yaw_rate = if generic.yaw_rate >= 0.0 {
                    generic.yaw_rate
                } else {
                    spin
                };
                let roll_rate = if generic.roll_rate >= 0.0 {
                    generic.roll_rate
                } else {
                    spin
                };
                let pitch_rate = if generic.pitch_rate >= 0.0 {
                    generic.pitch_rate
                } else {
                    spin
                };
                object.shock_yaw_rate = self.logic_random.next_real(-yaw_rate, yaw_rate);
                object.shock_roll_rate = self.logic_random.next_real(-roll_rate, roll_rate);
                object.shock_pitch_rate = self.logic_random.next_real(-pitch_rate, pitch_rate);
                let horizontal = 4.0 * generic.disposition_intensity;
                let vertical = 3.0 * generic.disposition_intensity;
                let force_x = self.logic_random.next_real(-horizontal, horizontal);
                let force_y = self.logic_random.next_real(-horizontal, horizontal);
                let force_z = self.logic_random.next_real(vertical * 0.33, vertical);
                // Draw in C++ Coord3D x/y/z order, then map the vertical
                // component to this host's Y-up force vector.
                let force = Vec3::new(force_x, force_z, force_y);
                object.apply_physics_force(force);
                object.set_orientation_with_partition_owner(
                    source_orientation,
                    &mut self.partition_manager,
                );
            }
            self.ocl_create_debris_reg
                .record_spawn(host_ocl_disposition_bits(generic.disposition));
        }
        OclExecutionOutcome::success(created)
    }

    /// Materialize FireFX and FireOCL events stamped when pending projectiles
    /// were drained. This is kept in GameLogic so both the normal host tick
    /// and GameWorld fire-spawn apply path use the same real object lifecycle.
    pub(crate) fn execute_pending_weapon_fire_ocls(&mut self) {
        self.execute_pending_weapon_fire_ocls_with_effects(
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn execute_pending_weapon_fire_ocls_with_effects(
        &mut self,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let fire_ocls = self.combat_system.take_fire_ocl();
        for fire in fire_ocls {
            // C++ Weapon.cpp handles FireFX before invoking FireOCL.  Do not
            // synthesize a generic muzzle event for a blank FX reference: the
            // parsed name is authoritative and a null FXList is genuinely no
            // visual effect.
            if !fire.fire_fx_name.trim().is_empty()
                && !fire.fire_fx_name.trim().eq_ignore_ascii_case("None")
            {
                let radius = host_fire_fx_override_radius(self.objects.get(&fire.shooter_id));
                let matrix = self
                    .objects
                    .get(&fire.shooter_id)
                    .map(|o| o.get_transform_matrix());
                let _ = self.spawn_receiving_weapon_fire_fx_with_effects(
                    fire.origin,
                    None,
                    self.frame,
                    fire.shooter_id,
                    None,
                    &fire.fire_fx_name,
                    "",
                    &fire.fire_ocl_name,
                    "",
                    0.0,
                    radius,
                    matrix,
                    presentation_effects,
                );
            }
            if !fire.fire_ocl_name.trim().is_empty()
                && !fire.fire_ocl_name.trim().eq_ignore_ascii_case("None")
            {
                let _ = self.execute_parsed_weapon_ocl_at_with_effects(
                    &fire.fire_ocl_name,
                    Some(fire.shooter_id),
                    fire.source_team,
                    fire.source_veterancy,
                    fire.source_orientation,
                    fire.source_velocity,
                    fire.origin,
                    presentation_effects,
                );
            }
        }
    }

    /// Flush real projectile impacts after either host- or GameWorld-owned
    /// flight integration.  Keeping this at the combat boundary ensures an
    /// OCL-only detonation cannot become a presentation-only no-op.
    pub(crate) fn flush_projectile_impact_fx(&mut self) {
        self.flush_projectile_impact_fx_with_effects(
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn flush_projectile_impact_fx_with_effects(
        &mut self,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let impacts = self.combat_system.take_impact_fx();
        for impact in impacts {
            // C++ executes ProjectileDetonationFX and
            // ProjectileDetonationOCL independently.  An OCL-only weapon
            // (common poison/radiation field pattern) therefore still reaches
            // the real host object-creation path.
            if impact.detonation_fx_name.is_empty() && impact.detonation_ocl_name.is_empty() {
                continue;
            }
            if !impact.detonation_ocl_name.is_empty() {
                let _ = self.execute_parsed_weapon_ocl_at_with_effects(
                    &impact.detonation_ocl_name,
                    Some(impact.shooter_id),
                    impact.source_team,
                    impact.source_veterancy,
                    impact.source_orientation,
                    impact.source_velocity,
                    impact.position,
                    presentation_effects,
                );
            }
            // C++ Weapon.cpp:903-939 plays ProjectileDetonationFX exactly once
            // (handleWeaponFireFX or a single doFXPos). Never invents a
            // fire-time MuzzleFlash at the crater and never unhides the
            // shooter's muzzle at detonation.
            if !impact.detonation_fx_name.trim().is_empty()
                && !impact
                    .detonation_fx_name
                    .trim()
                    .eq_ignore_ascii_case("None")
            {
                let radius = host_fire_fx_override_radius(self.objects.get(&impact.shooter_id));
                let matrix = self
                    .objects
                    .get(&impact.shooter_id)
                    .map(|o| o.get_transform_matrix());
                let _ = self.dispatch_fx_list_at_host_position_oriented_with_effects(
                    &impact.detonation_fx_name,
                    impact.position,
                    Some(impact.position),
                    0.0,
                    radius,
                    matrix,
                    presentation_effects,
                );
            }
        }
    }

    pub fn apply_nuke_cannon_primary_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        source_team: Team,
    ) -> (u32, bool) {
        self.apply_nuke_cannon_primary_at_with_effects(
            impact,
            source,
            source_team,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn apply_nuke_cannon_primary_at_with_effects(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        source_team: Team,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> (u32, bool) {
        use crate::game_logic::host_nuke_cannon::{
            MEDIUM_RADIATION_AUDIO, NUKE_CANNON_DAMAGE_TYPE, NUKE_CANNON_DEATH_TYPE,
            NUKE_CANNON_FIRE_AUDIO, is_legal_nuke_cannon_splash_target,
            nuke_cannon_primary_damage_at, nuke_cannon_splash_radius,
        };

        let impact_xz = (impact.x, impact.z);
        let max_r = nuke_cannon_splash_radius();
        let mut hits = 0u32;
        let mut any_destroyed = false;
        let mut destroy_ids: Vec<(ObjectId, Option<Team>)> = Vec::new();

        let candidates: Vec<(ObjectId, f32)> = self
            .objects
            .registered()
            .filter_map(|(id, obj)| {
                if source == Some(*id) {
                    return None;
                }
                let combat_kind = obj.is_kind_of(KindOf::Attackable)
                    || obj.is_kind_of(KindOf::Structure)
                    || obj.is_kind_of(KindOf::Infantry)
                    || obj.is_kind_of(KindOf::Vehicle)
                    || obj.is_kind_of(KindOf::Aircraft);
                if !is_legal_nuke_cannon_splash_target(
                    obj.is_alive(),
                    false,
                    obj.status.under_construction,
                    combat_kind,
                ) {
                    return None;
                }
                let pos = obj.get_position();
                let dist = {
                    let dx = impact_xz.0 - pos.x;
                    let dz = impact_xz.1 - pos.z;
                    (dx * dx + dz * dz).sqrt()
                };
                if dist <= max_r {
                    Some((*id, dist))
                } else {
                    None
                }
            })
            .collect();

        for (id, dist) in candidates {
            let dmg = nuke_cannon_primary_damage_at(dist);
            if dmg <= 0.0 {
                continue;
            }
            if let Some(obj) = self.objects.get_mut(&id) {
                let receiving_damage_target_id = obj.id;
                let destroyed = self.apply_residual_damage_owned_with_effects(
                    receiving_damage_target_id,
                    dmg,
                    source,
                    crate::game_logic::host_armor_residual::host_damage_type_from_residual_name(
                        NUKE_CANNON_DAMAGE_TYPE,
                    ),
                    crate::game_logic::host_armor_residual::host_death_type_from_residual_name(
                        NUKE_CANNON_DEATH_TYPE,
                    ),
                    None,
                    presentation_effects,
                );
                hits = hits.saturating_add(1);
                if destroyed {
                    any_destroyed = true;
                    destroy_ids.push((id, Some(source_team)));
                }
            }
        }

        for (id, killer) in destroy_ids {
            self.mark_object_for_destruction_with_effects(id, killer, presentation_effects);
        }

        self.nuke_cannon_residual.record_primary_blast(hits);

        // Medium radiation field residual at impact.
        let source_id = source.unwrap_or(ObjectId(0));
        let _ = self.nuke_cannon_residual.spawn_radiation_zone(
            source_id,
            source_team,
            impact,
            self.frame,
        );
        self.queue_audio_event(
            AudioEventRequest::new(MEDIUM_RADIATION_AUDIO)
                .with_position(impact)
                .with_priority(140),
        );
        self.queue_audio_event(
            AudioEventRequest::new(NUKE_CANNON_FIRE_AUDIO)
                .with_position(impact)
                .with_priority(160),
        );
        if let Some(sid) = source {
            let _ = self.combat_particles.spawn_weapon_fire_fx(
                self.objects
                    .get(&sid)
                    .map(|o| o.get_position())
                    .unwrap_or(impact),
                Some(impact),
                self.frame,
                sid,
                None,
            );
        }

        (hits, any_destroyed)
    }

    /// Apply Overlord/Helix portable gattling residual at impact.
    ///
    /// Independent portable rider (C++ HelixContain.cpp:340 RIDERS ALWAYS FIRE):
    /// slot 1 = AA `GattlingBuildingGunAir`; slot 0 = ground `GattlingBuildingGun`
    /// only — never stacked onto OverlordTankGun / HelixMinigun host damage.
    pub(crate) fn apply_overlord_gattling_residual_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
        slot: u8,
    ) -> (u32, bool) {
        self.apply_overlord_gattling_residual_at_with_effects(
            impact,
            source,
            intended_target,
            slot,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn apply_overlord_gattling_residual_at_with_effects(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
        slot: u8,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> (u32, bool) {
        use crate::game_logic::host_gattling_tank::has_chain_guns_upgrade;
        use crate::game_logic::host_overlord_addons::{
            OVERLORD_GATTLING_AIR_DAMAGE, OVERLORD_GATTLING_AIR_DAMAGE_TYPE,
            OVERLORD_GATTLING_DEATH_TYPE, OVERLORD_GATTLING_FIRE_AUDIO,
            OVERLORD_GATTLING_GROUND_DAMAGE_TYPE, is_legal_overlord_gattling_target,
            overlord_gattling_ground_damage,
        };

        let chain = source
            .and_then(|id| self.objects.get(&id))
            .map(|o| has_chain_guns_upgrade(&o.applied_upgrades))
            .unwrap_or(false);

        let (dmg, is_aa) = if slot == 1 {
            let mult = if chain { 1.25 } else { 1.0 };
            (OVERLORD_GATTLING_AIR_DAMAGE * mult, true)
        } else {
            (overlord_gattling_ground_damage(chain), false)
        };

        let mut hits = 0u32;
        let mut any_destroyed = false;
        let mut destroy_ids: Vec<(ObjectId, Option<Team>)> = Vec::new();
        let source_team = source.and_then(|id| self.objects.get(&id).map(|o| o.team));

        let tid = match intended_target {
            Some(id) => id,
            None => {
                // Pure residual acquire: nearest combat target near impact (XZ).
                let candidates: Vec<_> = self
                    .objects
                    .registered()
                    .map(|(&id, obj)| {
                        let combat_kind =
                            crate::game_logic::host_residual_acquire::residual_combat_kind(
                                obj.is_kind_of(KindOf::Attackable),
                                obj.is_kind_of(KindOf::Structure),
                                obj.is_kind_of(KindOf::Infantry),
                                obj.is_kind_of(KindOf::Vehicle),
                                obj.is_kind_of(KindOf::Aircraft),
                            );
                        crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                            id,
                            team: obj.team,
                            position: obj.get_position(),
                            is_alive: obj.is_alive(),
                            is_neutral: obj.team == Team::Neutral,
                            under_construction: obj.status.under_construction,
                            combat_kind,
                            effectively_stealthed: obj.is_effectively_stealthed(),
                            is_air: obj.is_kind_of(KindOf::Aircraft) || obj.status.airborne_target,
                            eject_invulnerable: obj.is_eject_invulnerable(),
                        }
                    })
                    .collect();
                match crate::game_logic::host_residual_acquire::pick_nearest_residual_target_xz(
                    source,
                    (impact.x, impact.z),
                    candidates,
                    12.0,
                    |c| {
                        is_legal_overlord_gattling_target(
                            true,
                            false,
                            c.under_construction,
                            c.combat_kind,
                        )
                    },
                ) {
                    Some((id, _, _)) => id,
                    None => {
                        if is_aa {
                            self.overlord_addons.record_gattling_aa_fire(0);
                        } else {
                            self.overlord_addons.record_gattling_ground_fire(0);
                        }
                        return (0, false);
                    }
                }
            }
        };

        if let Some(obj) = self.objects.get_mut(&tid) {
            let combat_kind = obj.is_kind_of(KindOf::Attackable)
                || obj.is_kind_of(KindOf::Structure)
                || obj.is_kind_of(KindOf::Infantry)
                || obj.is_kind_of(KindOf::Vehicle)
                || obj.is_kind_of(KindOf::Aircraft);
            if is_legal_overlord_gattling_target(
                obj.is_alive(),
                source == Some(tid),
                obj.status.under_construction,
                combat_kind,
            ) {
                let (dt_name, death_name) = if slot == 1 {
                    (
                        OVERLORD_GATTLING_AIR_DAMAGE_TYPE,
                        OVERLORD_GATTLING_DEATH_TYPE,
                    )
                } else {
                    (
                        OVERLORD_GATTLING_GROUND_DAMAGE_TYPE,
                        OVERLORD_GATTLING_DEATH_TYPE,
                    )
                };
                let receiving_damage_target_id = obj.id;
                let destroyed = self.apply_residual_damage_owned_with_effects(
                    receiving_damage_target_id,
                    dmg,
                    source,
                    crate::game_logic::host_armor_residual::host_damage_type_from_residual_name(
                        dt_name,
                    ),
                    crate::game_logic::host_armor_residual::host_death_type_from_residual_name(
                        death_name,
                    ),
                    None,
                    presentation_effects,
                );
                hits = 1;
                if destroyed {
                    any_destroyed = true;
                    destroy_ids.push((tid, source_team));
                }
            }
        }

        for (id, killer) in destroy_ids {
            self.mark_object_for_destruction_with_effects(id, killer, presentation_effects);
        }

        if is_aa {
            self.overlord_addons.record_gattling_aa_fire(hits);
        } else {
            self.overlord_addons.record_gattling_ground_fire(hits);
        }

        let muzzle = source
            .and_then(|id| self.objects.get(&id).map(|o| o.get_position()))
            .unwrap_or(impact);
        self.queue_audio_event(
            AudioEventRequest::new(OVERLORD_GATTLING_FIRE_AUDIO)
                .with_position(muzzle)
                .with_priority(140),
        );
        if let Some(sid) = source {
            let _ = self.combat_particles.spawn_weapon_fire_fx(
                muzzle,
                Some(impact),
                self.frame,
                sid,
                intended_target,
            );
        }

        (hits, any_destroyed)
    }

    /// C++ HelixContain.cpp:340 — portable gattling auto-acquires independently
    /// of the host chassis shot (not a piggyback +10 on OverlordTankGun).
    pub(in super::super) fn try_overlord_gattling_addon_independent_fire(
        &mut self,
        host_id: ObjectId,
    ) {
        self.try_overlord_gattling_addon_independent_fire_with_effects(
            host_id,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(in super::super) fn try_overlord_gattling_addon_independent_fire_with_effects(
        &mut self,
        host_id: ObjectId,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        use crate::game_logic::host_overlord_addons::{
            OVERLORD_GATTLING_AIR_RANGE, OVERLORD_GATTLING_GROUND_RANGE,
            is_legal_overlord_gattling_target, overlord_gattling_slot_for_air,
        };

        let current_time = self.frame as f32 * LOGIC_FRAME_TIMESTEP;
        let Some(host) = self.objects.get(&host_id) else {
            return;
        };
        if !host.has_overlord_gattling_residual() || !host.is_alive() {
            return;
        }
        if host.contained_by.is_some() {
            return;
        }
        let Some(aa) = host.weapon_slot(1) else {
            return;
        };
        if !Object::weapon_ready(aa, current_time) {
            return;
        }
        let team = host.team;
        let fire_pos = host.get_position();

        let candidates: Vec<_> = self
            .objects
            .registered()
            .filter(|(id, _)| **id != host_id)
            .map(|(id, obj)| {
                let combat_kind = crate::game_logic::host_residual_acquire::residual_combat_kind(
                    obj.is_kind_of(KindOf::Attackable),
                    obj.is_kind_of(KindOf::Structure),
                    obj.is_kind_of(KindOf::Infantry),
                    obj.is_kind_of(KindOf::Vehicle),
                    obj.is_kind_of(KindOf::Aircraft),
                );
                crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                    id: *id,
                    team: obj.team,
                    position: obj.get_position(),
                    is_alive: obj.is_alive(),
                    is_neutral: obj.team == Team::Neutral,
                    under_construction: obj.status.under_construction,
                    combat_kind,
                    effectively_stealthed: obj.is_effectively_stealthed(),
                    is_air: obj.is_kind_of(KindOf::Aircraft) || obj.status.airborne_target,
                    eject_invulnerable: obj.is_eject_invulnerable(),
                }
            })
            .collect();
        let best = crate::game_logic::host_residual_acquire::pick_nearest_residual_target(
            host_id,
            team,
            fire_pos,
            candidates,
            |is_air| {
                if is_air {
                    OVERLORD_GATTLING_AIR_RANGE
                } else {
                    OVERLORD_GATTLING_GROUND_RANGE
                }
            },
            |c| {
                c.is_alive
                    && c.team != team
                    && !c.is_neutral
                    && is_legal_overlord_gattling_target(
                        c.is_alive,
                        false,
                        c.under_construction,
                        c.combat_kind,
                    )
            },
        );
        let Some((target_id, _, _)) = best else {
            return;
        };
        let target_is_air = self
            .objects
            .get(&target_id)
            .map(|t| t.is_kind_of(KindOf::Aircraft) || t.status.airborne_target)
            .unwrap_or(false);
        let slot = overlord_gattling_slot_for_air(target_is_air);
        let impact = self
            .objects
            .get(&target_id)
            .map(|t| t.get_position())
            .unwrap_or(fire_pos);
        let (hits, _) = self.apply_overlord_gattling_residual_at_with_effects(
            impact,
            Some(host_id),
            Some(target_id),
            slot,
            presentation_effects,
        );
        if hits == 0 {
            return;
        }
        if let Some(host) = self.objects.get_mut(&host_id) {
            if let Some(w) = host.weapon_slot_mut(1) {
                crate::game_logic::Object::consume_ammo_on_fire(w, current_time);
            }
        }
    }

    /// Residual honesty: Overlord/Helix gattling addon install + fire path.
    pub fn honesty_overlord_gattling_ok(&self) -> bool {
        self.overlord_addons.honesty_gattling_install_ok()
            && self.overlord_addons.honesty_gattling_fire_ok()
    }

    /// Residual honesty: Overlord/Helix/Emperor propaganda addon residual.
    pub fn honesty_overlord_propaganda_ok(&self) -> bool {
        self.overlord_addons.honesty_propaganda_install_ok() || self.honesty_propaganda_heal_ok()
    }

    /// Residual honesty: Helix transport residual.
    pub fn honesty_helix_transport_ok(&self) -> bool {
        self.overlord_addons.honesty_helix_transport_ok()
    }

    pub fn overlord_addons(
        &self,
    ) -> &crate::game_logic::host_overlord_addons::HostOverlordAddonRegistry {
        &self.overlord_addons
    }

    /// Residual honesty: Nuke Cannon primary area + radiation residual.
    pub fn honesty_nuke_cannon_primary_ok(&self) -> bool {
        self.nuke_cannon_residual.honesty_primary_ok()
    }

    pub fn honesty_nuke_cannon_radiation_ok(&self) -> bool {
        self.nuke_cannon_residual.honesty_radiation_ok()
    }

    pub fn honesty_nuke_cannon_ok(&self) -> bool {
        self.nuke_cannon_residual.honesty_host_path_ok()
    }

    pub fn nuke_cannon_residual(
        &self,
    ) -> &crate::game_logic::host_nuke_cannon::HostNukeCannonRegistry {
        &self.nuke_cannon_residual
    }

    /// Infer Technical salvage tier from residual weapon stats / upgrade tags.
    pub(in super::super) fn technical_tier_from_object(
        obj: &Object,
    ) -> crate::game_logic::host_technical::TechnicalWeaponTier {
        use crate::game_logic::host_technical::TechnicalWeaponTier;
        if obj.has_upgrade_tag("WEAPONSET_CRATEUPGRADE_TWO")
            || obj.has_upgrade_tag("TechnicalCrateUpgradeTwo")
        {
            return TechnicalWeaponTier::Two;
        }
        if obj.has_upgrade_tag("WEAPONSET_CRATEUPGRADE_ONE")
            || obj.has_upgrade_tag("TechnicalCrateUpgradeOne")
        {
            return TechnicalWeaponTier::One;
        }
        // Infer from primary damage residual when tags absent.
        if let Some(w) = obj.weapon.as_ref() {
            if (w.damage - 50.0).abs() < 0.5 {
                return TechnicalWeaponTier::Two;
            }
            if (w.damage - 45.0).abs() < 0.5 {
                return TechnicalWeaponTier::One;
            }
        }
        TechnicalWeaponTier::Base
    }

    /// Apply residual salvage weapon tier to a Technical (crate upgrade residual).
    ///
    /// Fail-closed: not full SalvageCrate collate / W3D gunner subobject swap.
    pub fn apply_technical_weapon_tier(
        &mut self,
        object_id: ObjectId,
        tier: crate::game_logic::host_technical::TechnicalWeaponTier,
    ) -> bool {
        use crate::game_logic::host_technical::{
            TECHNICAL_TRANSPORT_SLOTS, delay_frames_to_reload_secs, is_technical_template,
            technical_weapon_for_tier, technical_weapon_name_for_tier, technical_weapon_stats,
        };
        use crate::game_logic::thing::ThingTemplate;

        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if !is_technical_template(&obj.template_name) {
            return false;
        }

        // Ensure passenger residual capacity is installed.
        if !obj.is_technical_style_container() {
            obj.install_technical_transport();
        }
        let _ = TECHNICAL_TRANSPORT_SLOTS;

        let name = technical_weapon_name_for_tier(tier);
        let (dmg, range, min_range, delay, _splash) = technical_weapon_stats(tier);
        let mut weapon = ThingTemplate::weapon_from_store(name)
            .unwrap_or_else(|| technical_weapon_for_tier(tier));
        // Force residual stats (store may lack min-range / reload).
        weapon.damage = dmg;
        weapon.range = range;
        weapon.min_range = min_range;
        weapon.reload_time = delay_frames_to_reload_secs(delay);
        weapon.can_target_ground = true;
        weapon.can_target_air = false;
        let _ = obj.replace_weapon_set_slot(0, Some(weapon));

        // Tag residual crate upgrade for tier inference.
        obj.applied_upgrades.remove("WEAPONSET_CRATEUPGRADE_ONE");
        obj.applied_upgrades.remove("WEAPONSET_CRATEUPGRADE_TWO");
        match tier {
            crate::game_logic::host_technical::TechnicalWeaponTier::One => {
                obj.applied_upgrades
                    .insert("WEAPONSET_CRATEUPGRADE_ONE".to_string());
            }
            crate::game_logic::host_technical::TechnicalWeaponTier::Two => {
                obj.applied_upgrades
                    .insert("WEAPONSET_CRATEUPGRADE_TWO".to_string());
            }
            crate::game_logic::host_technical::TechnicalWeaponTier::Base => {}
        }

        self.technical_residual_weapon_upgrades =
            self.technical_residual_weapon_upgrades.saturating_add(1);
        true
    }

    /// Apply Technical residual fire (MG direct or cannon/RPG splash).
    pub(in super::super) fn apply_technical_residual_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
    ) -> (u32, bool) {
        self.apply_technical_residual_at_with_effects(
            impact,
            source,
            intended_target,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(in super::super) fn apply_technical_residual_at_with_effects(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> (u32, bool) {
        use crate::game_logic::host_technical::{
            TECH_CANNON_DAMAGE_TYPE, TECH_CANNON_DEATH_TYPE, TECH_FIRE_AUDIO, TECH_MG_DAMAGE_TYPE,
            TECH_MG_DEATH_TYPE, TECH_RPG_DAMAGE_TYPE, TECH_RPG_DEATH_TYPE, TechnicalWeaponTier,
            is_legal_technical_splash_target, is_technical_template, technical_cannon_scatter_aim,
            technical_cannon_scatter_misses_infantry, technical_splash_damage_at,
            technical_weapon_stats,
        };

        let tier = source
            .and_then(|sid| self.objects.get(&sid))
            .map(Self::technical_tier_from_object)
            .unwrap_or(TechnicalWeaponTier::Base);
        let source_team = source
            .and_then(|sid| self.objects.get(&sid).map(|o| o.team))
            .unwrap_or(Team::Neutral);

        let (_dmg, _range, _min, _delay, splash) = technical_weapon_stats(tier);

        // C++ TechnicalCannonWeapon ScatterRadiusVsInfantry residual on instant apply.
        let mut impact = impact;
        let intended_is_infantry = intended_target
            .and_then(|id| self.objects.get(&id))
            .map(|o| o.is_kind_of(KindOf::Infantry))
            .unwrap_or(false);
        let cannon_tier =
            matches!(tier, TechnicalWeaponTier::One | TechnicalWeaponTier::Two) && splash > 0.0;
        if intended_is_infantry && cannon_tier {
            let seed = crate::game_logic::weapon_bootstrap::scatter_seed_for_shot(
                source.map(|s| s.0).unwrap_or(0),
                intended_target.map(|id| id.0).unwrap_or(0),
                self.frame,
            );
            let hit_r = intended_target
                .and_then(|id| self.objects.get(&id))
                .map(|o| {
                    if o.selection_radius > 0.0 {
                        o.selection_radius
                    } else {
                        crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS
                    }
                })
                .unwrap_or(crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS);
            let (new_impact, scattered) = technical_cannon_scatter_aim(impact, true, seed);
            if scattered {
                self.technical_cannon_scatter_applied =
                    self.technical_cannon_scatter_applied.saturating_add(1);
                impact = new_impact;
            }
            if technical_cannon_scatter_misses_infantry(true, seed, hit_r) {
                let intended_pos = intended_target
                    .and_then(|id| self.objects.get(&id))
                    .map(|o| o.get_position());
                if let Some(pos) = intended_pos {
                    let dx = impact.x - pos.x;
                    let dz = impact.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist > splash {
                        self.technical_cannon_scatter_misses =
                            self.technical_cannon_scatter_misses.saturating_add(1);
                    }
                }
            }
        }

        let impact_xz = (impact.x, impact.z);
        let mut hits = 0u32;
        let mut any_destroyed = false;
        let mut destroy_ids: Vec<(ObjectId, Option<Team>)> = Vec::new();

        // Collect candidates: intended always; splash ring when tier has radius.
        let candidates: Vec<(ObjectId, f32, bool)> = self
            .objects
            .registered()
            .filter_map(|(id, obj)| {
                if source == Some(*id) {
                    return None;
                }
                let combat_kind = obj.is_kind_of(KindOf::Attackable)
                    || obj.is_kind_of(KindOf::Structure)
                    || obj.is_kind_of(KindOf::Infantry)
                    || obj.is_kind_of(KindOf::Vehicle)
                    || obj.is_kind_of(KindOf::Aircraft);
                if !is_legal_technical_splash_target(
                    obj.is_alive(),
                    false,
                    obj.status.under_construction,
                    combat_kind,
                ) {
                    return None;
                }
                let pos = obj.get_position();
                let dist = {
                    let dx = impact_xz.0 - pos.x;
                    let dz = impact_xz.1 - pos.z;
                    (dx * dx + dz * dz).sqrt()
                };
                let is_intended = intended_target == Some(*id);
                // Scatter miss residual: intended infantry outside splash is not force-hit.
                if is_intended
                    && intended_is_infantry
                    && cannon_tier
                    && (splash <= 0.0 || dist > splash)
                {
                    return None;
                }
                if is_intended || (splash > 0.0 && dist <= splash) {
                    Some((*id, dist, is_intended))
                } else {
                    None
                }
            })
            .collect();

        for (id, dist, is_intended) in candidates {
            let dmg = technical_splash_damage_at(tier, is_intended, dist);
            if dmg <= 0.0 {
                continue;
            }
            if let Some(obj) = self.objects.get_mut(&id) {
                let (dt_name, death_name) = match tier {
                    TechnicalWeaponTier::Base => (TECH_MG_DAMAGE_TYPE, TECH_MG_DEATH_TYPE),
                    TechnicalWeaponTier::One => (TECH_CANNON_DAMAGE_TYPE, TECH_CANNON_DEATH_TYPE),
                    TechnicalWeaponTier::Two => (TECH_RPG_DAMAGE_TYPE, TECH_RPG_DEATH_TYPE),
                };
                let receiving_damage_target_id = obj.id;
                let destroyed = self.apply_residual_damage_owned_with_effects(
                    receiving_damage_target_id,
                    dmg,
                    source,
                    crate::game_logic::host_armor_residual::host_damage_type_from_residual_name(
                        dt_name,
                    ),
                    crate::game_logic::host_armor_residual::host_death_type_from_residual_name(
                        death_name,
                    ),
                    None,
                    presentation_effects,
                );
                hits = hits.saturating_add(1);
                if destroyed {
                    any_destroyed = true;
                    destroy_ids.push((id, Some(source_team)));
                }
            }
        }

        for (id, killer) in destroy_ids {
            self.mark_object_for_destruction_with_effects(id, killer, presentation_effects);
        }

        self.technical_residual_fires = self.technical_residual_fires.saturating_add(1);
        self.technical_residual_units_hit = self.technical_residual_units_hit.saturating_add(hits);

        self.queue_audio_event(
            AudioEventRequest::new(TECH_FIRE_AUDIO)
                .with_position(impact)
                .with_priority(150),
        );
        if let Some(sid) = source {
            let _ = self.combat_particles.spawn_weapon_fire_fx(
                self.objects
                    .get(&sid)
                    .map(|o| o.get_position())
                    .unwrap_or(impact),
                Some(impact),
                self.frame,
                sid,
                intended_target,
            );
            let _ = is_technical_template(
                &self
                    .objects
                    .get(&sid)
                    .map(|o| o.template_name.clone())
                    .unwrap_or_default(),
            );
        }

        (hits, any_destroyed)
    }

    /// Record Technical residual passenger load honesty (Enter residual).
    pub fn record_technical_residual_load(&mut self) {
        self.technical_residual_loads = self.technical_residual_loads.saturating_add(1);
    }

    /// Record Technical residual passenger unload honesty.
    pub fn record_technical_residual_unload(&mut self) {
        self.technical_residual_unloads = self.technical_residual_unloads.saturating_add(1);
    }

    /// Infer Marauder salvage tier from residual weapon reload / upgrade tags.
    pub(in super::super) fn marauder_tier_from_object(
        obj: &Object,
    ) -> crate::game_logic::host_marauder::MarauderWeaponTier {
        use crate::game_logic::host_marauder::MarauderWeaponTier;
        if obj.has_upgrade_tag("WEAPONSET_CRATEUPGRADE_TWO")
            || obj.has_upgrade_tag("MarauderCrateUpgradeTwo")
        {
            return MarauderWeaponTier::Two;
        }
        if obj.has_upgrade_tag("WEAPONSET_CRATEUPGRADE_ONE")
            || obj.has_upgrade_tag("MarauderCrateUpgradeOne")
        {
            return MarauderWeaponTier::One;
        }
        // Infer from reload residual when tags absent (faster = higher tier).
        if let Some(w) = obj.weapon.as_ref() {
            // Tier2 ~23/30s, Tier1 ~45/30s, Base ~60/30s.
            if w.reload_time <= (23.0 / 30.0) + 0.02 {
                return MarauderWeaponTier::Two;
            }
            if w.reload_time <= (45.0 / 30.0) + 0.02 {
                return MarauderWeaponTier::One;
            }
        }
        MarauderWeaponTier::Base
    }

    /// Apply BlackNapalm residual to an Inferno Cannon (PLAYER_UPGRADE fire field residual).
    ///
    /// Tags the unit so subsequent shell impacts spawn FireFieldUpgradedSmall
    /// residual (7.5 dmg/tick). Fail-closed: not HistoricBonus Firestorm matrix.
    pub fn apply_inferno_black_napalm_upgrade(&mut self, object_id: ObjectId) -> bool {
        use crate::game_logic::host_dragon_tank::UPGRADE_CHINA_BLACK_NAPALM;
        use crate::game_logic::host_inferno_cannon::is_inferno_cannon_template;
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if !is_inferno_cannon_template(&obj.template_name) {
            return false;
        }
        obj.applied_upgrades
            .insert(UPGRADE_CHINA_BLACK_NAPALM.to_string());
        self.inferno_black_napalm_residual_upgrades = self
            .inferno_black_napalm_residual_upgrades
            .saturating_add(1);
        true
    }

    /// Apply BlackNapalm residual to a Dragon Tank (PLAYER_UPGRADE flame residual).
    pub fn apply_dragon_black_napalm_upgrade(&mut self, object_id: ObjectId) -> bool {
        use crate::game_logic::host_dragon_tank::{
            UPGRADE_CHINA_BLACK_NAPALM, dragon_flame_weapon, is_dragon_tank_template,
        };
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if !is_dragon_tank_template(&obj.template_name) {
            return false;
        }
        obj.applied_upgrades
            .insert(UPGRADE_CHINA_BLACK_NAPALM.to_string());
        let _ = obj.replace_weapon_set_slot(0, Some(dragon_flame_weapon(true)));
        self.dragon_tank_residual_black_napalm_upgrades = self
            .dragon_tank_residual_black_napalm_upgrades
            .saturating_add(1);
        true
    }

    /// Apply Chain Guns residual to a Gattling Tank or Gattling Cannon structure
    /// (PLAYER_UPGRADE damage residual × 1.25).
    pub fn apply_gattling_chain_guns_upgrade(&mut self, object_id: ObjectId) -> bool {
        use crate::game_logic::host_base_defense::{
            gattling_building_air_weapon, gattling_building_ground_weapon,
            is_gattling_cannon_structure,
        };
        use crate::game_logic::host_gattling_tank::{
            GattlingFireLevel, UPGRADE_CHINA_CHAIN_GUNS, gattling_air_weapon,
            gattling_ground_weapon, is_gattling_tank_template,
        };
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        let is_tank = is_gattling_tank_template(&obj.template_name);
        let is_building = is_gattling_cannon_structure(&obj.template_name);
        if !is_tank && !is_building {
            return false;
        }
        obj.applied_upgrades
            .insert(UPGRADE_CHINA_CHAIN_GUNS.to_string());
        let level = GattlingFireLevel::from_u8(obj.continuous_fire_level);
        if is_tank {
            // Chain Guns is a C++ WeaponBonusUpgrade, so it must retain the
            // live weapon/barrel state rather than rebind the WeaponSet.
            obj.weapon = Some(gattling_ground_weapon(level, true));
            obj.secondary_weapon = Some(gattling_air_weapon(level, true));
            self.gattling_tank_residual_chain_gun_upgrades = self
                .gattling_tank_residual_chain_gun_upgrades
                .saturating_add(1);
        } else {
            obj.weapon = Some(gattling_building_ground_weapon(level, true));
            obj.secondary_weapon = Some(gattling_building_air_weapon(level, true));
            self.gattling_building_residual_chain_gun_upgrades = self
                .gattling_building_residual_chain_gun_upgrades
                .saturating_add(1);
        }
        true
    }

    /// Advance structure Gattling Cannon continuous-fire ramp residual after a shot.
    pub(in super::super) fn advance_gattling_building_continuous_fire(
        &mut self,
        attacker_id: ObjectId,
        target_id: Option<ObjectId>,
        slot: u8,
    ) {
        use crate::game_logic::host_base_defense::{
            GATTLING_BUILDING_RAPID_FIRE_AUDIO, gattling_building_air_weapon,
            gattling_building_coast_until_after_shot, gattling_building_ground_weapon,
            gattling_building_has_chain_guns, gattling_building_on_shot_fired,
        };
        use crate::game_logic::host_gattling_tank::GattlingFireLevel;

        let frame = self.frame;
        let Some(obj) = self.objects.get_mut(&attacker_id) else {
            return;
        };
        let prev_level = GattlingFireLevel::from_u8(obj.continuous_fire_level);
        let prev_consec = obj.consecutive_shots_at_target;
        let prev_victim = obj.consecutive_shot_target.map(|id| id.0);
        let new_victim = target_id.map(|id| id.0);
        let coast_until = obj.continuous_fire_coast_until_frame;

        let (new_level, consecutive, entered_fast) = gattling_building_on_shot_fired(
            prev_level,
            prev_consec,
            prev_victim,
            new_victim,
            frame,
            coast_until,
        );

        let chain = gattling_building_has_chain_guns(&obj.applied_upgrades);
        obj.continuous_fire_level = new_level.as_u8();
        obj.consecutive_shots_at_target = consecutive;
        // C++ coolDown resets the victim together with the count
        // (FiringTracker.cpp:319-321); consecutive == 0 marks that reset.
        obj.consecutive_shot_target = if consecutive == 0 { None } else { target_id };
        // C++ m_frameToStartCooldown = getPossibleNextShotFrame() + coast
        // (FiringTracker.cpp:106-110): the next-shot frame was stamped with
        // the FIRED shot's (pre-promotion) delay (Weapon.cpp:2645-2647;
        // speedUp runs after firing), so the deadline uses prev_level.
        obj.continuous_fire_coast_until_frame =
            gattling_building_coast_until_after_shot(frame, prev_level);

        // Rebind weapons with ramped reload residual.
        if let Some(w) = obj.weapon.as_mut() {
            let refreshed = gattling_building_ground_weapon(new_level, chain);
            w.damage = refreshed.damage;
            w.range = refreshed.range;
            w.reload_time = refreshed.reload_time;
            w.can_target_air = false;
            w.can_target_ground = true;
        }
        if let Some(w) = obj.secondary_weapon.as_mut() {
            let refreshed = gattling_building_air_weapon(new_level, chain);
            w.damage = refreshed.damage;
            w.range = refreshed.range;
            w.reload_time = refreshed.reload_time;
            w.can_target_air = true;
            w.can_target_ground = false;
        }

        if slot == 1 {
            self.gattling_building_residual_aa_fires =
                self.gattling_building_residual_aa_fires.saturating_add(1);
        } else {
            self.gattling_building_residual_ground_fires = self
                .gattling_building_residual_ground_fires
                .saturating_add(1);
        }
        if new_level == GattlingFireLevel::Mean && prev_level != GattlingFireLevel::Mean {
            self.gattling_building_residual_ramp_mean =
                self.gattling_building_residual_ramp_mean.saturating_add(1);
        }
        let became_fast = entered_fast
            || (new_level == GattlingFireLevel::Fast && prev_level != GattlingFireLevel::Fast);
        if became_fast {
            self.gattling_building_residual_ramp_fast =
                self.gattling_building_residual_ramp_fast.saturating_add(1);
            // C++ FiringTracker::speedUp MEAN→FAST: getPerUnitSound("VoiceRapidFire") + setObjectID.
            self.queue_resolved_per_unit_sound(
                attacker_id,
                GATTLING_BUILDING_RAPID_FIRE_AUDIO,
                true,
                false,
                None,
                140,
            );
        }
    }

    /// Advance Gattling continuous-fire ramp residual after a successful shot.
    pub(in super::super) fn advance_gattling_continuous_fire(
        &mut self,
        attacker_id: ObjectId,
        target_id: Option<ObjectId>,
        slot: u8,
    ) {
        use crate::game_logic::host_gattling_tank::{
            GATTLING_RAPID_FIRE_AUDIO, GattlingFireLevel, gattling_air_weapon,
            gattling_coast_until_after_shot, gattling_ground_weapon, gattling_on_shot_fired,
            has_chain_guns_upgrade,
        };

        let frame = self.frame;
        let Some(obj) = self.objects.get_mut(&attacker_id) else {
            return;
        };
        let prev_level = GattlingFireLevel::from_u8(obj.continuous_fire_level);
        let prev_consec = obj.consecutive_shots_at_target;
        let prev_victim = obj.consecutive_shot_target.map(|id| id.0);
        let new_victim = target_id.map(|id| id.0);
        let coast_until = obj.continuous_fire_coast_until_frame;

        let (new_level, consecutive, entered_fast) = gattling_on_shot_fired(
            prev_level,
            prev_consec,
            prev_victim,
            new_victim,
            frame,
            coast_until,
        );

        let chain = has_chain_guns_upgrade(&obj.applied_upgrades);
        obj.continuous_fire_level = new_level.as_u8();
        obj.consecutive_shots_at_target = consecutive;
        // C++ coolDown resets the victim together with the count
        // (FiringTracker.cpp:319-321); consecutive == 0 marks that reset.
        obj.consecutive_shot_target = if consecutive == 0 { None } else { target_id };
        // C++ m_frameToStartCooldown = getPossibleNextShotFrame() + coast
        // (FiringTracker.cpp:106-110): the next-shot frame was stamped with
        // the FIRED shot's (pre-promotion) delay (Weapon.cpp:2645-2647;
        // speedUp runs after firing), so the deadline uses prev_level.
        obj.continuous_fire_coast_until_frame = gattling_coast_until_after_shot(frame, prev_level);

        // Rebind weapons with ramped reload residual.
        if let Some(w) = obj.weapon.as_mut() {
            let refreshed = gattling_ground_weapon(new_level, chain);
            w.damage = refreshed.damage;
            w.range = refreshed.range;
            w.reload_time = refreshed.reload_time;
            w.can_target_air = false;
            w.can_target_ground = true;
        }
        if let Some(w) = obj.secondary_weapon.as_mut() {
            let refreshed = gattling_air_weapon(new_level, chain);
            w.damage = refreshed.damage;
            w.range = refreshed.range;
            w.reload_time = refreshed.reload_time;
            w.can_target_air = true;
            w.can_target_ground = false;
        }

        if slot == 1 {
            self.gattling_tank_residual_aa_fires =
                self.gattling_tank_residual_aa_fires.saturating_add(1);
        } else {
            self.gattling_tank_residual_ground_fires =
                self.gattling_tank_residual_ground_fires.saturating_add(1);
        }
        if new_level == GattlingFireLevel::Mean && prev_level != GattlingFireLevel::Mean {
            self.gattling_tank_residual_ramp_mean =
                self.gattling_tank_residual_ramp_mean.saturating_add(1);
        }
        let became_fast = entered_fast
            || (new_level == GattlingFireLevel::Fast && prev_level != GattlingFireLevel::Fast);
        if became_fast {
            self.gattling_tank_residual_ramp_fast =
                self.gattling_tank_residual_ramp_fast.saturating_add(1);
            // C++ FiringTracker::speedUp MEAN→FAST: getPerUnitSound("VoiceRapidFire") + setObjectID.
            self.queue_resolved_per_unit_sound(
                attacker_id,
                GATTLING_RAPID_FIRE_AUDIO,
                true,
                false,
                None,
                140,
            );
        }
    }

    /// Apply Dragon Tank flame residual at impact (primary + secondary splash).
    ///
    /// Returns (units_hit, any_destroyed).
    pub(in super::super) fn apply_dragon_flame_residual_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
    ) -> (u32, bool) {
        self.apply_dragon_flame_residual_at_with_effects(
            impact,
            source,
            intended_target,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(in super::super) fn apply_dragon_flame_residual_at_with_effects(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> (u32, bool) {
        use crate::game_logic::host_dragon_tank::{
            DRAGON_DAMAGE_TYPE, DRAGON_DEATH_TYPE, DRAGON_FIRE_AUDIO, DRAGON_SECONDARY_RADIUS,
            dragon_flame_damage_at, has_black_napalm_upgrade, is_legal_dragon_flame_target,
        };

        let upgraded = source
            .and_then(|id| self.objects.get(&id))
            .map(|o| has_black_napalm_upgrade(&o.applied_upgrades))
            .unwrap_or(false);
        let impact_xz = (impact.x, impact.z);
        let mut hits = 0u32;
        let mut any_destroyed = false;
        let mut destroy_ids: Vec<(ObjectId, Option<Team>)> = Vec::new();
        let source_team = source.and_then(|id| self.objects.get(&id).map(|o| o.team));

        let candidates: Vec<(ObjectId, f32, bool)> = self
            .objects
            .registered()
            .filter_map(|(id, obj)| {
                if source == Some(*id) {
                    return None;
                }
                let combat_kind = obj.is_kind_of(KindOf::Attackable)
                    || obj.is_kind_of(KindOf::Structure)
                    || obj.is_kind_of(KindOf::Infantry)
                    || obj.is_kind_of(KindOf::Vehicle)
                    || obj.is_kind_of(KindOf::Aircraft);
                if !is_legal_dragon_flame_target(
                    obj.is_alive(),
                    false,
                    obj.status.under_construction,
                    combat_kind,
                ) {
                    return None;
                }
                let pos = obj.get_position();
                let dist = {
                    let dx = impact_xz.0 - pos.x;
                    let dz = impact_xz.1 - pos.z;
                    (dx * dx + dz * dz).sqrt()
                };
                let is_intended = intended_target == Some(*id);
                if is_intended || dist <= DRAGON_SECONDARY_RADIUS {
                    Some((*id, dist, is_intended))
                } else {
                    None
                }
            })
            .collect();

        for (id, dist, is_intended) in candidates {
            let dmg = dragon_flame_damage_at(upgraded, is_intended, dist);
            if dmg <= 0.0 {
                continue;
            }
            if let Some(obj) = self.objects.get_mut(&id) {
                let receiving_damage_target_id = obj.id;
                let destroyed = self.apply_residual_damage_owned_with_effects(
                    receiving_damage_target_id,
                    dmg,
                    source,
                    crate::game_logic::host_armor_residual::host_damage_type_from_residual_name(
                        DRAGON_DAMAGE_TYPE,
                    ),
                    crate::game_logic::host_armor_residual::host_death_type_from_residual_name(
                        DRAGON_DEATH_TYPE,
                    ),
                    None,
                    presentation_effects,
                );
                hits = hits.saturating_add(1);
                if destroyed {
                    any_destroyed = true;
                    destroy_ids.push((id, source_team));
                }
            }
        }

        for (id, killer) in destroy_ids {
            self.mark_object_for_destruction_with_effects(id, killer, presentation_effects);
        }

        self.dragon_tank_residual_fires = self.dragon_tank_residual_fires.saturating_add(1);
        self.dragon_tank_residual_units_hit =
            self.dragon_tank_residual_units_hit.saturating_add(hits);

        self.queue_audio_event(
            AudioEventRequest::new(DRAGON_FIRE_AUDIO)
                .with_position(impact)
                .with_priority(150),
        );
        if let Some(sid) = source {
            let _ = self.combat_particles.spawn_weapon_fire_fx(
                self.objects
                    .get(&sid)
                    .map(|o| o.get_position())
                    .unwrap_or(impact),
                Some(impact),
                self.frame,
                sid,
                intended_target,
            );
        }

        (hits, any_destroyed)
    }

    /// Apply Gattling Tank residual hit at impact (single-target residual).
    ///
    /// Returns (units_hit, any_destroyed). Continuous-fire ramp advances in
    /// `advance_gattling_continuous_fire` after the fire path records last_fire_time.
    pub(in super::super) fn apply_gattling_tank_residual_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
        slot: u8,
    ) -> (u32, bool) {
        self.apply_gattling_tank_residual_at_with_effects(
            impact,
            source,
            intended_target,
            slot,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(in super::super) fn apply_gattling_tank_residual_at_with_effects(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
        slot: u8,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> (u32, bool) {
        use crate::game_logic::host_gattling_tank::{
            GATTLING_AIR_DAMAGE, GATTLING_AIR_DAMAGE_TYPE, GATTLING_DEATH_TYPE,
            GATTLING_FIRE_AUDIO, GATTLING_GROUND_DAMAGE, GATTLING_GROUND_DAMAGE_TYPE,
            gattling_damage_with_chain_guns, has_chain_guns_upgrade, is_legal_gattling_target,
        };

        let (base_dmg, chain) = source
            .and_then(|id| self.objects.get(&id))
            .map(|o| {
                let chain = has_chain_guns_upgrade(&o.applied_upgrades);
                let base = if slot == 1 {
                    GATTLING_AIR_DAMAGE
                } else {
                    GATTLING_GROUND_DAMAGE
                };
                (base, chain)
            })
            .unwrap_or((GATTLING_GROUND_DAMAGE, false));
        let dmg = gattling_damage_with_chain_guns(base_dmg, chain);

        let mut hits = 0u32;
        let mut any_destroyed = false;
        let mut destroy_ids: Vec<(ObjectId, Option<Team>)> = Vec::new();
        let source_team = source.and_then(|id| self.objects.get(&id).map(|o| o.team));

        let tid = match intended_target {
            Some(id) => id,
            None => {
                // Pure residual acquire: nearest combat target near impact (XZ).
                let candidates: Vec<_> = self
                    .objects
                    .registered()
                    .map(|(&id, obj)| {
                        let combat_kind =
                            crate::game_logic::host_residual_acquire::residual_combat_kind(
                                obj.is_kind_of(KindOf::Attackable),
                                obj.is_kind_of(KindOf::Structure),
                                obj.is_kind_of(KindOf::Infantry),
                                obj.is_kind_of(KindOf::Vehicle),
                                obj.is_kind_of(KindOf::Aircraft),
                            );
                        crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                            id,
                            team: obj.team,
                            position: obj.get_position(),
                            is_alive: obj.is_alive(),
                            is_neutral: obj.team == Team::Neutral,
                            under_construction: obj.status.under_construction,
                            combat_kind,
                            effectively_stealthed: obj.is_effectively_stealthed(),
                            is_air: obj.is_kind_of(KindOf::Aircraft) || obj.status.airborne_target,
                            eject_invulnerable: obj.is_eject_invulnerable(),
                        }
                    })
                    .collect();
                match crate::game_logic::host_residual_acquire::pick_nearest_residual_target_xz(
                    source,
                    (impact.x, impact.z),
                    candidates,
                    12.0,
                    |c| is_legal_gattling_target(true, false, c.under_construction, c.combat_kind),
                ) {
                    Some((id, _, _)) => id,
                    None => {
                        self.queue_audio_event(
                            AudioEventRequest::new(GATTLING_FIRE_AUDIO)
                                .with_position(impact)
                                .with_priority(140),
                        );
                        return (0, false);
                    }
                }
            }
        };

        if let Some(obj) = self.objects.get_mut(&tid) {
            if is_legal_gattling_target(
                obj.is_alive(),
                source == Some(tid),
                obj.status.under_construction,
                true,
            ) {
                let (dt_name, death_name) = if slot == 1 {
                    (GATTLING_AIR_DAMAGE_TYPE, GATTLING_DEATH_TYPE)
                } else {
                    (GATTLING_GROUND_DAMAGE_TYPE, GATTLING_DEATH_TYPE)
                };
                let receiving_damage_target_id = obj.id;
                let destroyed = self.apply_residual_damage_owned_with_effects(
                    receiving_damage_target_id,
                    dmg,
                    source,
                    crate::game_logic::host_armor_residual::host_damage_type_from_residual_name(
                        dt_name,
                    ),
                    crate::game_logic::host_armor_residual::host_death_type_from_residual_name(
                        death_name,
                    ),
                    None,
                    presentation_effects,
                );
                hits = 1;
                if destroyed {
                    any_destroyed = true;
                    destroy_ids.push((tid, source_team));
                }
            }
        }

        for (id, killer) in destroy_ids {
            self.mark_object_for_destruction_with_effects(id, killer, presentation_effects);
        }

        self.queue_audio_event(
            AudioEventRequest::new(GATTLING_FIRE_AUDIO)
                .with_position(impact)
                .with_priority(140),
        );
        if let Some(sid) = source {
            let _ = self.combat_particles.spawn_weapon_fire_fx(
                self.objects
                    .get(&sid)
                    .map(|o| o.get_position())
                    .unwrap_or(impact),
                Some(impact),
                self.frame,
                sid,
                Some(tid),
            );
        }

        (hits, any_destroyed)
    }

    /// Apply residual salvage fire-rate tier to a Marauder (crate upgrade residual).
    ///
    /// Fail-closed: not full SalvageCrate collate / W3D turret subobject swap.
    pub fn apply_marauder_weapon_tier(
        &mut self,
        object_id: ObjectId,
        tier: crate::game_logic::host_marauder::MarauderWeaponTier,
    ) -> bool {
        use crate::game_logic::host_marauder::{
            delay_frames_to_reload_secs, is_marauder_template, marauder_weapon_for_tier,
            marauder_weapon_name_for_tier, marauder_weapon_stats,
        };
        use crate::game_logic::thing::ThingTemplate;

        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if !is_marauder_template(&obj.template_name) {
            return false;
        }

        let name = marauder_weapon_name_for_tier(tier);
        let (dmg, range, delay, _splash, speed) = marauder_weapon_stats(tier);
        let mut weapon = ThingTemplate::weapon_from_store(name)
            .unwrap_or_else(|| marauder_weapon_for_tier(tier));
        weapon.damage = dmg;
        weapon.range = range;
        weapon.min_range = 0.0;
        weapon.reload_time = delay_frames_to_reload_secs(delay);
        weapon.projectile_speed = speed;
        weapon.can_target_ground = true;
        weapon.can_target_air = false;
        let _ = obj.replace_weapon_set_slot(0, Some(weapon));

        obj.applied_upgrades.remove("WEAPONSET_CRATEUPGRADE_ONE");
        obj.applied_upgrades.remove("WEAPONSET_CRATEUPGRADE_TWO");
        match tier {
            crate::game_logic::host_marauder::MarauderWeaponTier::One => {
                obj.applied_upgrades
                    .insert("WEAPONSET_CRATEUPGRADE_ONE".to_string());
            }
            crate::game_logic::host_marauder::MarauderWeaponTier::Two => {
                obj.applied_upgrades
                    .insert("WEAPONSET_CRATEUPGRADE_TWO".to_string());
            }
            crate::game_logic::host_marauder::MarauderWeaponTier::Base => {}
        }

        self.marauder_residual_weapon_upgrades =
            self.marauder_residual_weapon_upgrades.saturating_add(1);
        true
    }

    /// Apply Marauder residual fire (primary on intended + small splash radius).
    /// C++ MarauderTankShell DumbProjectile residual.
    pub fn spawn_marauder_shell_projectile(
        &mut self,
        source_id: ObjectId,
        from: glam::Vec3,
        aim: glam::Vec3,
        intended: Option<ObjectId>,
        weapon_speed: f32,
    ) -> Option<ObjectId> {
        self.spawn_marauder_shell_projectile_with_effects(
            source_id,
            from,
            aim,
            intended,
            weapon_speed,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn spawn_marauder_shell_projectile_with_effects(
        &mut self,
        source_id: ObjectId,
        from: glam::Vec3,
        aim: glam::Vec3,
        intended: Option<ObjectId>,
        weapon_speed: f32,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_marauder::{
            MARAUDER_SHELL_MAX_HEALTH, MARAUDER_TANK_SHELL, marauder_shell_flight_frames,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        if !self.templates.contains_key(MARAUDER_TANK_SHELL) {
            let mut t = ThingTemplate::new(MARAUDER_TANK_SHELL);
            t.add_kind_of(KindOf::Projectile)
                .set_health(MARAUDER_SHELL_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates.insert(MARAUDER_TANK_SHELL.to_string(), t);
        }
        let team = self
            .objects
            .get(&source_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);

        // C++ ScatterRadiusVsInfantry residual on MarauderTankGun vs infantry.
        let target_is_infantry = intended
            .and_then(|id| self.objects.get(&id))
            .map(|o| o.is_kind_of(KindOf::Infantry))
            .unwrap_or(false);
        let seed = crate::game_logic::weapon_bootstrap::scatter_seed_for_shot(
            source_id.0,
            intended.map(|id| id.0).unwrap_or(0),
            self.frame,
        );
        let (aim, scattered) =
            crate::game_logic::host_marauder::marauder_scatter_aim(aim, target_is_infantry, seed);
        if scattered {
            self.marauder_scatter_applied = self.marauder_scatter_applied.saturating_add(1);
        }
        if target_is_infantry {
            let hit_r = intended
                .and_then(|id| self.objects.get(&id))
                .map(|o| {
                    if o.selection_radius > 0.0 {
                        o.selection_radius
                    } else {
                        crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS
                    }
                })
                .unwrap_or(crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS);
            let intended_pos = intended
                .and_then(|id| self.objects.get(&id))
                .map(|o| o.get_position());
            if crate::game_logic::host_marauder::marauder_scatter_misses_infantry(true, seed, hit_r)
            {
                if let Some(pos) = intended_pos {
                    let dx = aim.x - pos.x;
                    let dz = aim.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist > crate::game_logic::host_marauder::MARAUDER_SPLASH_RADIUS {
                        self.marauder_scatter_misses =
                            self.marauder_scatter_misses.saturating_add(1);
                    }
                }
            }
        }

        let mut start = from;
        start.y = start.y.max(aim.y) + 4.0;
        let pid = self.create_object_with_script_effects(
            MARAUDER_TANK_SHELL,
            team,
            start,
            None,
            presentation_effects,
        )?;
        let frames = marauder_shell_flight_frames(start, aim, weapon_speed).max(1);
        if let Some(o) = self.objects.get_mut(&pid) {
            o.marauder_shell_projectile = true;
            o.marauder_shell_from = Some([start.x, start.y, start.z]);
            o.marauder_shell_aim = Some([aim.x, aim.y, aim.z]);
            o.marauder_shell_launch_frame = Some(self.frame);
            o.marauder_shell_flight_frames = frames;
            o.marauder_shell_intended = intended.map(|id| id.0);
            o.marauder_shell_weapon_speed = weapon_speed;
            o.note_producer(source_id);
            o.set_body_max_health_with_mode(
                MARAUDER_SHELL_MAX_HEALTH,
                crate::game_logic::object::MaxHealthChange::FullyHeal,
            );
            self.health_events
                .record_applied_heal(o.id, o.health.current);
        }
        self.marauder_shells_spawned = self.marauder_shells_spawned.saturating_add(1);
        Some(pid)
    }

    pub fn update_marauder_shell_projectiles(&mut self) {
        self.update_marauder_shell_projectiles_with_effects(
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn update_marauder_shell_projectiles_with_effects(
        &mut self,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        use crate::game_logic::host_marauder::marauder_shell_bezier_point;
        let frame = self.frame;
        let flying: Vec<ObjectId> = self
            .objects
            .registered()
            .filter_map(|(id, o)| {
                if o.marauder_shell_projectile && o.is_alive() {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        let mut impact: Vec<(ObjectId, Option<ObjectId>, Option<ObjectId>, glam::Vec3)> =
            Vec::new();
        for id in flying {
            let (source, intended, from, aim, launch, frames) = {
                let Some(o) = self.objects.get(&id) else {
                    continue;
                };
                let from = o
                    .marauder_shell_from
                    .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                    .unwrap_or_else(|| o.get_position());
                let aim = o
                    .marauder_shell_aim
                    .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                    .unwrap_or(from);
                (
                    o.producer_id,
                    o.marauder_shell_intended.map(ObjectId),
                    from,
                    aim,
                    o.marauder_shell_launch_frame.unwrap_or(frame),
                    o.marauder_shell_flight_frames.max(1),
                )
            };
            let elapsed = frame.saturating_sub(launch);
            let t = (elapsed as f32 / frames as f32).clamp(0.0, 1.0);
            let pos = marauder_shell_bezier_point(from, aim, t);
            if let Some(o) = self.objects.get_mut(&id) {
                let prev = o.get_position();
                o.set_position_with_effects(
                    pos,
                    self.frame,
                    &mut presentation_effects.reborrow_for_object(&mut self.partition_manager),
                );
                let d = pos - prev;
                if d.length_squared() > 1.0e-6 {
                    o.set_orientation_with_partition_owner(
                        d.z.atan2(d.x),
                        &mut self.partition_manager,
                    );
                }
            }
            if elapsed >= frames {
                impact.push((id, source, intended, aim));
            }
        }
        for (id, source, intended, pos) in impact {
            let team = self.objects.get(&id).map(|o| o.team);
            if let Some(o) = self.objects.get_mut(&id) {
                // Wave 752: under damage authority, do not zero host HP mid-frame
                // (dual with GW HP writeback). Project lethal via damage log + flags.
                {
                    o.internal_change_health(-o.health.current);
                }
                o.status.destroyed = true;
                o.status.effectively_dead = true;
                o.marauder_shell_projectile = false;
                o.set_position_with_effects(
                    pos,
                    self.frame,
                    &mut presentation_effects.reborrow_for_object(&mut self.partition_manager),
                );
            }
            let _ = self.apply_marauder_residual_at_with_effects(
                pos,
                source,
                intended,
                presentation_effects,
            );
            self.mark_object_for_destruction_with_effects(id, team, presentation_effects);
        }
    }

    pub fn honesty_marauder_shell_projectile_ok(&self) -> bool {
        self.marauder_shells_spawned > 0
    }

    pub fn apply_marauder_residual_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
    ) -> (u32, bool) {
        self.apply_marauder_residual_at_with_effects(
            impact,
            source,
            intended_target,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn apply_marauder_residual_at_with_effects(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> (u32, bool) {
        use crate::game_logic::host_marauder::{
            MARAUDER_DAMAGE_TYPE, MARAUDER_DEATH_TYPE, MARAUDER_FIRE_AUDIO, MARAUDER_SPLASH_RADIUS,
            is_legal_marauder_splash_target, is_marauder_template, marauder_scatter_aim,
            marauder_scatter_misses_infantry, marauder_splash_damage_at,
        };

        // Fire-rate tier residual is encoded on the weapon; damage is constant across tiers.
        let _tier = source
            .and_then(|sid| self.objects.get(&sid))
            .map(Self::marauder_tier_from_object);
        let source_team = source
            .and_then(|sid| self.objects.get(&sid).map(|o| o.team))
            .unwrap_or(Team::Neutral);

        // C++ MarauderTankGun ScatterRadiusVsInfantry residual on instant apply.
        let mut impact = impact;
        let intended_is_infantry = intended_target
            .and_then(|id| self.objects.get(&id))
            .map(|o| o.is_kind_of(KindOf::Infantry))
            .unwrap_or(false);
        if intended_is_infantry {
            let seed = crate::game_logic::weapon_bootstrap::scatter_seed_for_shot(
                source.map(|s| s.0).unwrap_or(0),
                intended_target.map(|id| id.0).unwrap_or(0),
                self.frame,
            );
            let hit_r = intended_target
                .and_then(|id| self.objects.get(&id))
                .map(|o| {
                    if o.selection_radius > 0.0 {
                        o.selection_radius
                    } else {
                        crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS
                    }
                })
                .unwrap_or(crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS);
            let (new_impact, scattered) = marauder_scatter_aim(impact, true, seed);
            if scattered {
                self.marauder_scatter_applied = self.marauder_scatter_applied.saturating_add(1);
                impact = new_impact;
            }
            if marauder_scatter_misses_infantry(true, seed, hit_r) {
                let intended_pos = intended_target
                    .and_then(|id| self.objects.get(&id))
                    .map(|o| o.get_position());
                if let Some(pos) = intended_pos {
                    let dx = impact.x - pos.x;
                    let dz = impact.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist > MARAUDER_SPLASH_RADIUS {
                        self.marauder_scatter_misses =
                            self.marauder_scatter_misses.saturating_add(1);
                    }
                }
            }
        }

        let impact_xz = (impact.x, impact.z);
        let mut hits = 0u32;
        let mut any_destroyed = false;
        let mut destroy_ids: Vec<(ObjectId, Option<Team>)> = Vec::new();

        let candidates: Vec<(ObjectId, f32, bool)> = self
            .objects
            .registered()
            .filter_map(|(id, obj)| {
                if source == Some(*id) {
                    return None;
                }
                let combat_kind = obj.is_kind_of(KindOf::Attackable)
                    || obj.is_kind_of(KindOf::Structure)
                    || obj.is_kind_of(KindOf::Infantry)
                    || obj.is_kind_of(KindOf::Vehicle)
                    || obj.is_kind_of(KindOf::Aircraft);
                if !is_legal_marauder_splash_target(
                    obj.is_alive(),
                    false,
                    obj.status.under_construction,
                    combat_kind,
                ) {
                    return None;
                }
                let pos = obj.get_position();
                let dist = {
                    let dx = impact_xz.0 - pos.x;
                    let dz = impact_xz.1 - pos.z;
                    (dx * dx + dz * dz).sqrt()
                };
                let is_intended = intended_target == Some(*id);
                // Scatter miss residual: intended infantry outside splash is not force-hit.
                if is_intended && intended_is_infantry && dist > MARAUDER_SPLASH_RADIUS {
                    return None;
                }
                if is_intended || dist <= MARAUDER_SPLASH_RADIUS {
                    Some((*id, dist, is_intended))
                } else {
                    None
                }
            })
            .collect();

        for (id, dist, is_intended) in candidates {
            let dmg = marauder_splash_damage_at(is_intended, dist);
            if dmg <= 0.0 {
                continue;
            }
            if let Some(obj) = self.objects.get_mut(&id) {
                let receiving_damage_target_id = obj.id;
                let destroyed = self.apply_residual_damage_owned_with_effects(
                    receiving_damage_target_id,
                    dmg,
                    source,
                    crate::game_logic::host_armor_residual::host_damage_type_from_residual_name(
                        MARAUDER_DAMAGE_TYPE,
                    ),
                    crate::game_logic::host_armor_residual::host_death_type_from_residual_name(
                        MARAUDER_DEATH_TYPE,
                    ),
                    None,
                    presentation_effects,
                );
                hits = hits.saturating_add(1);
                if destroyed {
                    any_destroyed = true;
                    destroy_ids.push((id, Some(source_team)));
                }
            }
        }

        for (id, killer) in destroy_ids {
            self.mark_object_for_destruction_with_effects(id, killer, presentation_effects);
        }

        self.marauder_residual_fires = self.marauder_residual_fires.saturating_add(1);
        self.marauder_residual_units_hit = self.marauder_residual_units_hit.saturating_add(hits);

        self.queue_audio_event(
            AudioEventRequest::new(MARAUDER_FIRE_AUDIO)
                .with_position(impact)
                .with_priority(150),
        );
        if let Some(sid) = source {
            let _ = self.combat_particles.spawn_weapon_fire_fx(
                self.objects
                    .get(&sid)
                    .map(|o| o.get_position())
                    .unwrap_or(impact),
                Some(impact),
                self.frame,
                sid,
                intended_target,
            );
            let _ = is_marauder_template(
                &self
                    .objects
                    .get(&sid)
                    .map(|o| o.template_name.clone())
                    .unwrap_or_default(),
            );
        }

        (hits, any_destroyed)
    }

    /// Apply Scorpion salvage gun tier residual (primary damage 20 → 25).
    pub fn apply_scorpion_salvage_tier(
        &mut self,
        object_id: ObjectId,
        tier: crate::game_logic::host_scorpion::ScorpionSalvageTier,
    ) -> bool {
        use crate::game_logic::host_scorpion::{
            ScorpionSalvageTier, has_ap_rockets_upgrade, has_scorpion_rocket_upgrade,
            is_scorpion_template, scorpion_gun_weapon, scorpion_missile_weapon,
        };

        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if !is_scorpion_template(&obj.template_name) {
            return false;
        }

        obj.applied_upgrades.remove("WEAPONSET_CRATEUPGRADE_ONE");
        obj.applied_upgrades.remove("WEAPONSET_CRATEUPGRADE_TWO");
        match tier {
            ScorpionSalvageTier::One => {
                obj.applied_upgrades
                    .insert("WEAPONSET_CRATEUPGRADE_ONE".to_string());
            }
            ScorpionSalvageTier::Two => {
                obj.applied_upgrades
                    .insert("WEAPONSET_CRATEUPGRADE_TWO".to_string());
            }
            ScorpionSalvageTier::Base => {}
        }

        let last_fire = obj.weapon.as_ref().map(|w| w.last_fire_time).unwrap_or(0.0);
        let mut gun = scorpion_gun_weapon(tier);
        gun.last_fire_time = last_fire;
        let _ = obj.replace_weapon_set_slot(0, Some(gun));

        if has_scorpion_rocket_upgrade(&obj.applied_upgrades) {
            let ap = has_ap_rockets_upgrade(&obj.applied_upgrades);
            let sec_last = obj
                .secondary_weapon
                .as_ref()
                .map(|w| w.last_fire_time)
                .unwrap_or(0.0);
            let mut missile = scorpion_missile_weapon(ap, tier.dual_missile_clip());
            missile.last_fire_time = sec_last;
            // Salvage changes the selected C++ WeaponSet.  Even when AP
            // Rockets also contributes its in-place stats, this concrete
            // secondary Weapon instance was rebuilt and starts with a fresh
            // barrel cursor.
            let _ = obj.replace_weapon_set_slot(1, Some(missile));
        }

        self.scorpion_residual_salvage_upgrades =
            self.scorpion_residual_salvage_upgrades.saturating_add(1);
        true
    }

    /// Equip Scorpion Rocket secondary residual (Upgrade_GLAScorpionRocket).
    pub fn apply_scorpion_rocket_upgrade(&mut self, object_id: ObjectId) -> bool {
        use crate::game_logic::host_scorpion::{
            UPGRADE_GLA_SCORPION_ROCKET, has_ap_rockets_upgrade, is_scorpion_template,
            salvage_tier_from_upgrades, scorpion_missile_weapon,
        };

        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if !is_scorpion_template(&obj.template_name) {
            return false;
        }
        obj.applied_upgrades
            .insert(UPGRADE_GLA_SCORPION_ROCKET.to_string());
        let tier = salvage_tier_from_upgrades(&obj.applied_upgrades);
        let ap = has_ap_rockets_upgrade(&obj.applied_upgrades);
        let _ = obj.replace_weapon_set_slot(
            1,
            Some(scorpion_missile_weapon(ap, tier.dual_missile_clip())),
        );
        self.scorpion_residual_rocket_upgrades =
            self.scorpion_residual_rocket_upgrades.saturating_add(1);
        true
    }

    /// Apply AP Rockets residual damage mult to Scorpion missile secondary.
    pub fn apply_scorpion_ap_rockets_upgrade(&mut self, object_id: ObjectId) -> bool {
        use crate::game_logic::host_scorpion::{
            UPGRADE_GLA_AP_ROCKETS, has_scorpion_rocket_upgrade, is_scorpion_template,
            salvage_tier_from_upgrades, scorpion_missile_weapon,
        };

        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if !is_scorpion_template(&obj.template_name) {
            return false;
        }
        obj.applied_upgrades
            .insert(UPGRADE_GLA_AP_ROCKETS.to_string());
        if has_scorpion_rocket_upgrade(&obj.applied_upgrades) {
            let tier = salvage_tier_from_upgrades(&obj.applied_upgrades);
            let sec_last = obj
                .secondary_weapon
                .as_ref()
                .map(|w| w.last_fire_time)
                .unwrap_or(0.0);
            let mut missile = scorpion_missile_weapon(true, tier.dual_missile_clip());
            missile.last_fire_time = sec_last;
            // AP Rockets is a C++ WeaponBonus upgrade layered onto the
            // selected Scorpion Rocket WeaponSet; preserve its live barrel
            // cursor while refreshing the boosted stats.
            obj.secondary_weapon = Some(missile);
        }
        true
    }

    /// C++ ScorpionTankShell DumbProjectile residual.
    pub fn spawn_scorpion_shell_projectile(
        &mut self,
        source_id: ObjectId,
        from: glam::Vec3,
        aim: glam::Vec3,
        intended: Option<ObjectId>,
        slot: u8,
    ) -> Option<ObjectId> {
        self.spawn_scorpion_shell_projectile_with_effects(
            source_id,
            from,
            aim,
            intended,
            slot,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn spawn_scorpion_shell_projectile_with_effects(
        &mut self,
        source_id: ObjectId,
        from: glam::Vec3,
        aim: glam::Vec3,
        intended: Option<ObjectId>,
        slot: u8,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_scorpion::{
            SCORPION_SHELL_MAX_HEALTH, SCORPION_TANK_SHELL, scorpion_shell_flight_frames,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        if !self.templates.contains_key(SCORPION_TANK_SHELL) {
            let mut t = ThingTemplate::new(SCORPION_TANK_SHELL);
            t.add_kind_of(KindOf::Projectile)
                .set_health(SCORPION_SHELL_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates.insert(SCORPION_TANK_SHELL.to_string(), t);
        }
        let team = self
            .objects
            .get(&source_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);

        // C++ ScatterRadiusVsInfantry residual on ScorpionTankGun vs infantry.
        let target_is_infantry = intended
            .and_then(|id| self.objects.get(&id))
            .map(|o| o.is_kind_of(KindOf::Infantry))
            .unwrap_or(false);
        let seed = crate::game_logic::weapon_bootstrap::scatter_seed_for_shot(
            source_id.0,
            intended.map(|id| id.0).unwrap_or(0),
            self.frame,
        );
        let (aim, scattered) =
            crate::game_logic::host_scorpion::scorpion_scatter_aim(aim, target_is_infantry, seed);
        if scattered {
            self.scorpion_scatter_applied = self.scorpion_scatter_applied.saturating_add(1);
        }
        if target_is_infantry {
            let hit_r = intended
                .and_then(|id| self.objects.get(&id))
                .map(|o| {
                    if o.selection_radius > 0.0 {
                        o.selection_radius
                    } else {
                        crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS
                    }
                })
                .unwrap_or(crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS);
            let intended_pos = intended
                .and_then(|id| self.objects.get(&id))
                .map(|o| o.get_position());
            if crate::game_logic::host_scorpion::scorpion_scatter_misses_infantry(true, seed, hit_r)
            {
                if let Some(pos) = intended_pos {
                    let dx = aim.x - pos.x;
                    let dz = aim.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist > crate::game_logic::host_scorpion::SCORPION_GUN_SPLASH_RADIUS {
                        self.scorpion_scatter_misses =
                            self.scorpion_scatter_misses.saturating_add(1);
                    }
                }
            }
        }

        let mut start = from;
        start.y = start.y.max(aim.y) + 4.0;
        let pid = self.create_object_with_script_effects(
            SCORPION_TANK_SHELL,
            team,
            start,
            None,
            presentation_effects,
        )?;
        let frames = scorpion_shell_flight_frames(start, aim).max(1);
        if let Some(o) = self.objects.get_mut(&pid) {
            o.scorpion_shell_projectile = true;
            o.scorpion_shell_from = Some([start.x, start.y, start.z]);
            o.scorpion_shell_aim = Some([aim.x, aim.y, aim.z]);
            o.scorpion_shell_launch_frame = Some(self.frame);
            o.scorpion_shell_flight_frames = frames;
            o.scorpion_shell_slot = slot;
            o.note_producer(source_id);
            o.set_body_max_health_with_mode(
                SCORPION_SHELL_MAX_HEALTH,
                crate::game_logic::object::MaxHealthChange::FullyHeal,
            );
            self.health_events
                .record_applied_heal(o.id, o.health.current);
        }
        self.scorpion_shells_spawned = self.scorpion_shells_spawned.saturating_add(1);
        Some(pid)
    }

    pub fn update_scorpion_shell_projectiles(&mut self) {
        self.update_scorpion_shell_projectiles_with_effects(
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn update_scorpion_shell_projectiles_with_effects(
        &mut self,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        use crate::game_logic::host_scorpion::scorpion_shell_bezier_point;
        let frame = self.frame;
        let flying: Vec<ObjectId> = self
            .objects
            .registered()
            .filter_map(|(id, o)| {
                if o.scorpion_shell_projectile && o.is_alive() {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        let mut impact: Vec<(ObjectId, Option<ObjectId>, glam::Vec3, u8)> = Vec::new();
        for id in flying {
            let (source, from, aim, launch, frames, slot) = {
                let Some(o) = self.objects.get(&id) else {
                    continue;
                };
                let from = o
                    .scorpion_shell_from
                    .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                    .unwrap_or_else(|| o.get_position());
                let aim = o
                    .scorpion_shell_aim
                    .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                    .unwrap_or(from);
                (
                    o.producer_id,
                    from,
                    aim,
                    o.scorpion_shell_launch_frame.unwrap_or(frame),
                    o.scorpion_shell_flight_frames.max(1),
                    o.scorpion_shell_slot,
                )
            };
            let elapsed = frame.saturating_sub(launch);
            let t = (elapsed as f32 / frames as f32).clamp(0.0, 1.0);
            let pos = scorpion_shell_bezier_point(from, aim, t);
            if let Some(o) = self.objects.get_mut(&id) {
                let prev = o.get_position();
                o.set_position_with_effects(
                    pos,
                    self.frame,
                    &mut presentation_effects.reborrow_for_object(&mut self.partition_manager),
                );
                let d = pos - prev;
                if d.length_squared() > 1.0e-6 {
                    o.set_orientation_with_partition_owner(
                        d.z.atan2(d.x),
                        &mut self.partition_manager,
                    );
                }
            }
            if elapsed >= frames {
                impact.push((id, source, aim, slot));
            }
        }
        for (id, source, pos, slot) in impact {
            let team = self.objects.get(&id).map(|o| o.team);
            if let Some(o) = self.objects.get_mut(&id) {
                // Wave 752: under damage authority, do not zero host HP mid-frame
                // (dual with GW HP writeback). Project lethal via damage log + flags.
                {
                    o.internal_change_health(-o.health.current);
                }
                o.status.destroyed = true;
                o.status.effectively_dead = true;
                o.scorpion_shell_projectile = false;
                o.set_position_with_effects(
                    pos,
                    self.frame,
                    &mut presentation_effects.reborrow_for_object(&mut self.partition_manager),
                );
            }
            let _ = self.apply_scorpion_residual_at_with_effects(
                pos,
                source,
                None,
                slot,
                presentation_effects,
            );
            self.mark_object_for_destruction_with_effects(id, team, presentation_effects);
        }
    }

    /// C++ ScorpionMissile ProjectileObject residual.
    pub fn spawn_scorpion_missile_projectile(
        &mut self,
        source_id: ObjectId,
        from: glam::Vec3,
        aim: glam::Vec3,
        intended: Option<ObjectId>,
        slot: u8,
    ) -> Option<ObjectId> {
        self.spawn_scorpion_missile_projectile_with_effects(
            source_id,
            from,
            aim,
            intended,
            slot,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn spawn_scorpion_missile_projectile_with_effects(
        &mut self,
        source_id: ObjectId,
        from: glam::Vec3,
        aim: glam::Vec3,
        intended: Option<ObjectId>,
        slot: u8,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_scorpion::{
            SCORPION_MISSILE, SCORPION_MISSILE_FUEL_FRAMES, SCORPION_MISSILE_INITIAL_VELOCITY,
            SCORPION_MISSILE_MAX_HEALTH, SCORPION_MISSILE_PROJECTILE_SPEED,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        if !self.templates.contains_key(SCORPION_MISSILE) {
            let mut t = ThingTemplate::new(SCORPION_MISSILE);
            t.add_kind_of(KindOf::Projectile)
                .set_health(SCORPION_MISSILE_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates.insert(SCORPION_MISSILE.to_string(), t);
        }
        let team = self
            .objects
            .get(&source_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);

        // C++ ScatterRadiusVsInfantry residual on ScorpionMissileWeapon vs infantry.
        let target_is_infantry = intended
            .and_then(|id| self.objects.get(&id))
            .map(|o| o.is_kind_of(KindOf::Infantry))
            .unwrap_or(false);
        let seed = crate::game_logic::weapon_bootstrap::scatter_seed_for_shot(
            source_id.0,
            intended.map(|id| id.0).unwrap_or(0),
            self.frame,
        );
        let (aim, scattered) =
            crate::game_logic::host_scorpion::scorpion_scatter_aim(aim, target_is_infantry, seed);
        if scattered {
            self.scorpion_scatter_applied = self.scorpion_scatter_applied.saturating_add(1);
        }
        if target_is_infantry {
            let hit_r = intended
                .and_then(|id| self.objects.get(&id))
                .map(|o| {
                    if o.selection_radius > 0.0 {
                        o.selection_radius
                    } else {
                        crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS
                    }
                })
                .unwrap_or(crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS);
            let intended_pos = intended
                .and_then(|id| self.objects.get(&id))
                .map(|o| o.get_position());
            if crate::game_logic::host_scorpion::scorpion_scatter_misses_infantry(true, seed, hit_r)
            {
                if let Some(pos) = intended_pos {
                    let dx = aim.x - pos.x;
                    let dz = aim.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist > crate::game_logic::host_scorpion::SCORPION_MISSILE_SECONDARY_RADIUS {
                        self.scorpion_scatter_misses =
                            self.scorpion_scatter_misses.saturating_add(1);
                    }
                }
            }
        }

        let mut start = from;
        start.y = start.y.max(aim.y) + 6.0;
        let pid = self.create_object_with_script_effects(
            SCORPION_MISSILE,
            team,
            start,
            None,
            presentation_effects,
        )?;
        let launch = SCORPION_MISSILE_INITIAL_VELOCITY / 30.0;
        let to_aim = aim - start;
        let dist = to_aim.length().max(0.001);
        let dir = to_aim / dist;
        if let Some(o) = self.objects.get_mut(&pid) {
            o.scorpion_missile_projectile = true;
            o.scorpion_missile_aim = Some([aim.x, aim.y, aim.z]);
            o.scorpion_missile_intended = intended.map(|id| id.0);
            o.scorpion_missile_travelled = 0.0;
            o.scorpion_missile_fuel_expires_frame =
                Some(self.frame.saturating_add(SCORPION_MISSILE_FUEL_FRAMES));
            o.scorpion_missile_slot = slot;
            o.note_producer(source_id);
            o.set_body_max_health_with_mode(
                SCORPION_MISSILE_MAX_HEALTH,
                crate::game_logic::object::MaxHealthChange::FullyHeal,
            );
            self.health_events
                .record_applied_heal(o.id, o.health.current);
            o.movement.velocity = dir * launch;
            o.set_orientation_with_partition_owner(dir.z.atan2(dir.x), &mut self.partition_manager);
        }
        let _ = SCORPION_MISSILE_PROJECTILE_SPEED; // cruise used in update
        self.scorpion_missiles_spawned = self.scorpion_missiles_spawned.saturating_add(1);
        Some(pid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::ThingTemplate;

    #[test]
    fn fire_ocl_never_uses_a_foreign_list_or_foreign_object_template() {
        const NAME: &str = "OCL_WeaponOwnerNegativeControl";
        let text = |child: &str| {
            format!(
                "ObjectCreationList {NAME}\n CreateObject\n ObjectNames = {child}\n Count = 1\n End\nEnd\n"
            )
        };
        let foreign = gamelogic::object_creation_list::store::parse_object_creation_lists_from_ini(
            &text("ForeignChild"),
        )
        .unwrap();
        for (name, list) in foreign {
            gamelogic::helpers::TheObjectCreationListStore::register_object_creation_list(
                &name, list,
            );
        }
        let mut first = GameLogic::new();
        let mut peer = GameLogic::new();
        for world in [&mut first, &mut peer] {
            for name in ["SameSource", "OwnedChild", "ForeignChild"] {
                world
                    .templates
                    .insert(name.into(), ThingTemplate::new(name));
            }
        }
        peer.apply_receiving_ocl_overrides(&[text("OwnedChild")])
            .unwrap();
        let first_id = first
            .create_object("SameSource", Team::USA, Vec3::ZERO)
            .unwrap();
        let peer_id = peer
            .create_object("SameSource", Team::USA, Vec3::ZERO)
            .unwrap();
        assert_eq!(first_id, peer_id);
        let execute = |world: &mut GameLogic, id| {
            world.execute_parsed_weapon_ocl_at(
                NAME,
                Some(id),
                Team::USA,
                VeterancyLevel::Rookie,
                0.0,
                Vec3::ZERO,
                Vec3::ZERO,
            )
        };
        assert!(execute(&mut first, first_id).is_empty());
        let children = execute(&mut peer, peer_id);
        assert_eq!(children.len(), 1);
        assert_eq!(peer.objects[&children[0]].template_name, "OwnedChild");
        peer.templates.remove("OwnedChild");
        assert!(execute(&mut peer, peer_id).is_empty());
    }

    #[test]
    fn retail_fire_field_ocl_creates_the_authored_object_for_the_source_team() {
        // Use the production asset filesystem's configured roots and BIG reader.
        // The compiled source directory and private runtime CWD need not contain
        // any extracted licensed INIs. Parse the owned retail bytes using the
        // existing OCL parser; absence is a genuine admission failure, not a skip.
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("retail asset IO runtime");
        let mut assets = crate::assets::archive::ArchiveFileSystem::new();
        runtime
            .block_on(assets.init())
            .expect("retail asset filesystem must initialize");
        let bytes = assets
            .open_file_sync("Data/INI/ObjectCreationList.ini")
            .expect("configured retail ObjectCreationList.ini must be readable");
        let text = std::str::from_utf8(&bytes)
            .expect("retail ObjectCreationList.ini must contain valid UTF-8");
        let mut logic = GameLogic::new();
        logic
            .apply_receiving_ocl_overrides(&[text.to_owned()])
            .expect("retail ObjectCreationList.ini must parse");
        logic.templates.insert(
            "__OclBridgeSource".into(),
            ThingTemplate::new("__OclBridgeSource"),
        );
        // The test supplies the exact existing Object identity; production
        // obtains it from the parsed asset definition before creating it.
        logic.templates.insert(
            "FireFieldSmall".into(),
            ThingTemplate::new("FireFieldSmall"),
        );
        let source = logic
            .create_object("__OclBridgeSource", Team::China, Vec3::new(10.0, 3.0, 20.0))
            .expect("source object");

        let created = logic.execute_parsed_weapon_ocl_at(
            "OCL_FireFieldSmall",
            Some(source),
            Team::China,
            VeterancyLevel::Veteran,
            0.0,
            Vec3::ZERO,
            Vec3::new(10.0, 3.0, 20.0),
        );

        assert_eq!(created.len(), 1);
        let field = logic.objects.get(&created[0]).expect("created field");
        assert_eq!(field.template_name, "FireFieldSmall");
        assert_eq!(field.team, Team::China);
        assert_eq!(field.producer_id, Some(source));
        assert_eq!(field.get_position(), Vec3::new(10.0, 3.0, 20.0));
    }

    #[test]
    fn mixed_ocl_still_creates_supported_nuggets_when_one_needs_leftover_physics() {
        // C++ ObjectCreationList.cpp:1524-1534 createInternal runs every nugget.
        // A leftover-physics CreateDebris must not drop the CreateObject spawn.
        let ini = r#"
ObjectCreationList OCL_HostMixedParity
  CreateObject
    ObjectNames = FireFieldSmall
    Count = 1
  End
  CreateDebris
    ModelNames = EXRockChunk
    Count = 1
    Disposition = SEND_IT_FLYING
    ExtraFriction = -0.3
  End
End
"#;
        let mut logic = GameLogic::new();
        logic
            .apply_receiving_ocl_overrides(&[ini.to_owned()])
            .expect("mixed OCL must parse");
        logic.templates.insert(
            "__OclBridgeSource".into(),
            ThingTemplate::new("__OclBridgeSource"),
        );
        logic.templates.insert(
            "FireFieldSmall".into(),
            ThingTemplate::new("FireFieldSmall"),
        );
        let source = logic
            .create_object("__OclBridgeSource", Team::China, Vec3::new(4.0, 1.0, 8.0))
            .expect("source object");

        let created = logic.execute_parsed_weapon_ocl_at(
            "OCL_HostMixedParity",
            Some(source),
            Team::China,
            VeterancyLevel::Rookie,
            0.0,
            Vec3::ZERO,
            Vec3::new(4.0, 1.0, 8.0),
        );

        assert_eq!(created.len(), 1);
        let field = logic.objects.get(&created[0]).expect("created field");
        assert_eq!(field.template_name, "FireFieldSmall");
        assert_eq!(field.team, Team::China);
    }

    /// C++ ObjectCreationList.cpp:1302-1305 — OCL create uses the source
    /// controlling player, not `unique_player_id_for_team` (None in 2v2).
    #[test]
    fn weapon_ocl_create_uses_source_owner_player_id_in_2v2() {
        let ini = r#"
ObjectCreationList OCL_HostOwnerParity
  CreateObject
    ObjectNames = FireFieldSmall
    Count = 1
  End
End
"#;
        let mut logic = GameLogic::new();
        logic
            .apply_receiving_ocl_overrides(&[ini.to_owned()])
            .expect("owner OCL must parse");
        logic.add_player(Player::new(3, Team::China, "ChinaA", true));
        logic.add_player(Player::new(7, Team::China, "ChinaB", false));
        assert_eq!(logic.unique_player_id_for_team(Team::China), None);

        logic.templates.insert(
            "__OclOwnerSource".into(),
            ThingTemplate::new("__OclOwnerSource"),
        );
        logic.templates.insert(
            "FireFieldSmall".into(),
            ThingTemplate::new("FireFieldSmall"),
        );
        let source = logic
            .create_object_for_player("__OclOwnerSource", 7, Vec3::new(10.0, 3.0, 20.0))
            .expect("source object");
        assert_eq!(
            logic.objects.get(&source).and_then(|o| o.owner_player_id),
            Some(7)
        );

        let created = logic.execute_parsed_weapon_ocl_at(
            "OCL_HostOwnerParity",
            Some(source),
            Team::China,
            VeterancyLevel::Veteran,
            0.0,
            Vec3::ZERO,
            Vec3::new(10.0, 3.0, 20.0),
        );

        assert_eq!(created.len(), 1);
        let field = logic.objects.get(&created[0]).expect("created field");
        assert_eq!(field.template_name, "FireFieldSmall");
        assert_eq!(field.team, Team::China);
        assert_eq!(field.owner_player_id, Some(7));
        assert_eq!(field.producer_id, Some(source));
    }

    /// C++ ObjectCreationList.cpp:996-1005 — inherit only if isTrainable
    /// and transferObjectName with the source script name.
    #[test]
    fn ocl_inherit_veterancy_only_if_trainable_and_transfers_script_name() {
        let ini = r#"
ObjectCreationList OCL_HostInheritVetParity
  CreateObject
    ObjectNames = OclEjectedPilot
    Count = 1
    InheritsVeterancy = Yes
  End
End
ObjectCreationList OCL_HostInheritFieldParity
  CreateObject
    ObjectNames = FireFieldSmall
    Count = 1
    InheritsVeterancy = Yes
  End
End
"#;
        let mut logic = GameLogic::new();
        logic
            .apply_receiving_ocl_overrides(&[ini.to_owned()])
            .expect("inherit OCL must parse");
        let mut source_tpl = ThingTemplate::new("__OclVetSource");
        source_tpl.is_trainable = true;
        logic.templates.insert("__OclVetSource".into(), source_tpl);

        let mut pilot = ThingTemplate::new("OclEjectedPilot");
        pilot.is_trainable = true;
        logic.templates.insert("OclEjectedPilot".into(), pilot);
        logic.templates.insert(
            "FireFieldSmall".into(),
            ThingTemplate::new("FireFieldSmall"),
        );

        let source = logic
            .create_object("__OclVetSource", Team::USA, Vec3::new(2.0, 0.0, 4.0))
            .expect("source");
        logic
            .with_object_veterancy_feedback(source, |src, audio, feedback| {
                src.name = "PilotOne".into();
                src.set_min_veterancy_level(VeterancyLevel::Elite, audio, feedback);
            })
            .expect("actual OCL source owner");

        logic.initialize_receiving_named_cache_for_fixture();
        assert_eq!(
            logic.host_object_id_by_script_name("PilotOne"),
            Some(source)
        );
        let created = logic.execute_parsed_weapon_ocl_at(
            "OCL_HostInheritVetParity",
            Some(source),
            Team::USA,
            VeterancyLevel::Elite,
            0.0,
            Vec3::ZERO,
            Vec3::new(2.0, 0.0, 4.0),
        );
        assert_eq!(created.len(), 1);
        let pilot_id = created[0];
        let spawned = logic.objects.get(&pilot_id).expect("pilot");
        assert_eq!(spawned.experience.level, VeterancyLevel::Elite);
        assert_eq!(spawned.name, "PilotOne");
        assert_eq!(
            logic.objects.get(&source).map(|o| o.name.as_str()),
            Some("PilotOne"),
            "CPP transferObjectName preserves the donor's field"
        );
        assert_eq!(
            logic.host_object_id_by_script_name("PilotOne"),
            Some(pilot_id)
        );

        let field_src = logic
            .create_object("__OclVetSource", Team::USA, Vec3::new(8.0, 0.0, 4.0))
            .expect("field source");
        logic
            .with_object_veterancy_feedback(field_src, |src, audio, feedback| {
                src.name = "FieldSrc".into();
                src.set_min_veterancy_level(VeterancyLevel::Elite, audio, feedback);
            })
            .expect("actual OCL source owner");
        let created_field = logic.execute_parsed_weapon_ocl_at(
            "OCL_HostInheritFieldParity",
            Some(field_src),
            Team::USA,
            VeterancyLevel::Elite,
            0.0,
            Vec3::ZERO,
            Vec3::new(8.0, 0.0, 4.0),
        );
        assert_eq!(created_field.len(), 1);
        let field = logic.objects.get(&created_field[0]).expect("field");
        assert!(!field.is_trainable());
        assert_eq!(field.experience.level, VeterancyLevel::Rookie);
        assert!(field.name.is_empty());
        assert_eq!(
            logic.objects.get(&field_src).map(|o| o.name.as_str()),
            Some("FieldSrc")
        );
    }

    /// C++ HelixContain.cpp:340 — portable gattling fires without host shot.
    #[test]
    fn overlord_gattling_independent_acquire_not_stacked_primary() {
        use crate::game_logic::{AIState, KindOf, Team};
        let mut logic = GameLogic::new();
        let mut overlord = ThingTemplate::new("ChinaTankOverlord");
        overlord
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(1100.0);
        logic
            .templates
            .insert("ChinaTankOverlord".to_string(), overlord);
        let mut enemy = ThingTemplate::new("UsaRanger");
        enemy
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(200.0);
        logic.templates.insert("UsaRanger".to_string(), enemy);

        let tank = logic
            .create_object("ChinaTankOverlord", Team::China, Vec3::new(0.0, 0.0, 0.0))
            .expect("overlord");
        {
            let o = logic.host_object_mut(tank).unwrap();
            o.install_overlord_gattling_addon();
            o.set_ai_state(
                AIState::Idle,
                &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
            );
            o.target = None;
            if let Some(w) = o.weapon_slot_mut(1) {
                w.last_fire_time = -10.0;
                w.reload_time = 0.1;
            }
        }
        let victim = logic
            .create_object("UsaRanger", Team::USA, Vec3::new(20.0, 0.0, 0.0))
            .expect("victim");
        let hp_before = logic.host_object(victim).unwrap().health.current;
        logic.set_current_frame(30);
        logic.try_overlord_gattling_addon_independent_fire(tank);
        let hp_after = logic.host_object(victim).unwrap().health.current;
        let dealt = hp_before - hp_after;
        assert!(
            (dealt - crate::game_logic::host_overlord_addons::OVERLORD_GATTLING_GROUND_DAMAGE)
                .abs()
                < 0.2,
            "independent gattling deals 10 not stacked primary+10, dealt={dealt}"
        );
        assert!(logic.overlord_addons.gattling_ground_fires > 0);
    }

    #[test]
    fn fade_in_nugget_spawns_and_starts_drawable_fade() {
        let ini = r#"
ObjectCreationList OCL_HostFadeInParity
  CreateObject
    ObjectNames = FadeDebris
    Count = 1
    FadeIn = Yes
    FadeTime = 1000
  End
End
"#;
        let mut logic = GameLogic::new();
        logic
            .apply_receiving_ocl_overrides(&[ini.to_owned()])
            .expect("fade OCL must parse");
        logic.frame = 40;
        logic.templates.insert(
            "__OclBridgeSource".into(),
            ThingTemplate::new("__OclBridgeSource"),
        );
        logic
            .templates
            .insert("FadeDebris".into(), ThingTemplate::new("FadeDebris"));
        let source = logic
            .create_object("__OclBridgeSource", Team::USA, Vec3::new(1.0, 0.0, 1.0))
            .expect("source object");

        let created = logic.execute_parsed_weapon_ocl_at(
            "OCL_HostFadeInParity",
            Some(source),
            Team::USA,
            VeterancyLevel::Rookie,
            0.0,
            Vec3::ZERO,
            Vec3::new(1.0, 0.0, 1.0),
        );
        assert_eq!(created.len(), 1);
        let debris = logic.objects.get(&created[0]).expect("faded debris");
        assert_eq!(
            debris.drawable_fade_mode,
            crate::game_logic::DRAWABLE_FADE_IN
        );
        assert_eq!(debris.drawable_fade_start_frame, 40);
        assert!(debris.drawable_fade_frames > 0);
        assert!((debris.drawable_fade_opacity(40) - 0.0).abs() < 1e-5);
        assert!(debris.drawable_fade_opacity(40 + debris.drawable_fade_frames) > 0.99);
    }
}
