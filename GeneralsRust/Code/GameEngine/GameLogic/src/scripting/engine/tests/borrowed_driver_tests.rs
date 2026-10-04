use super::*;

struct FlagObserver<'a> {
    engine: &'a ScriptEngine,
    observed: Vec<(bool, bool, bool)>,
}

impl ScriptExecutionDriver for FlagObserver<'_> {
    fn after_action(&mut self) -> GameLogicResult<()> {
        self.observed.push((
            self.engine
                .get_flag("driver_before")
                .is_some_and(|flag| flag.value),
            self.engine
                .get_flag("driver_nested")
                .is_some_and(|flag| flag.value),
            self.engine
                .get_flag("driver_after")
                .is_some_and(|flag| flag.value),
        ));
        Ok(())
    }
}

#[test]
fn nested_subroutine_reuses_borrowed_driver_before_next_outer_instruction() {
    // CPP ScriptEngine.cpp:7609–7652: callSubroutine is an immediate
    // ScriptEngine action in the outer linked action walk.
    let _guard = crate::test_sync::lock();
    let mut engine = ScriptEngine::new().unwrap();
    let mut outer = set_flag_action("driver_before");
    let mut call = call_subroutine_action("DriverNested");
    call.next_action = Some(set_flag_action("driver_after"));
    outer.next_action = Some(call);
    let mut root = Script::new();
    root.script_name = "DriverOuter".to_string();
    root.is_one_shot = true;
    root.condition = Some(always_true_condition());
    root.action = Some(outer);
    let mut list = ScriptList::new();
    list.append_script(Box::new(root));
    list.append_script(Box::new(subroutine(
        "DriverNested",
        set_flag_action("driver_nested"),
    )));
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    let mut context = ScriptContext::new();
    context.current_frame = 37;
    let mut driver = FlagObserver {
        engine: &engine,
        observed: Vec::new(),
    };
    engine.update_with_driver(context, &mut driver).unwrap();
    assert_eq!(
        driver.observed,
        vec![
            (true, false, false),
            (true, true, false),
            (true, true, false),
            (true, true, true),
        ]
    );
}

#[test]
fn false_action_chain_notifies_driver_between_every_instruction() {
    let _guard = crate::test_sync::lock();
    let mut engine = ScriptEngine::new().unwrap();
    let mut root = Script::new();
    root.script_name = "DriverFalseBranch".to_string();
    // No conditions is false in the original executor.
    let mut action = set_flag_action("driver_before");
    action.next_action = Some(set_flag_action("driver_after"));
    root.action_false = Some(action);
    root.is_one_shot = true;
    let mut list = ScriptList::new();
    list.append_script(Box::new(root));
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    let mut context = ScriptContext::new();
    context.current_frame = 37;
    let mut driver = FlagObserver {
        engine: &engine,
        observed: Vec::new(),
    };
    engine.update_with_driver(context, &mut driver).unwrap();
    assert_eq!(
        driver.observed,
        vec![(true, false, false), (true, false, true)]
    );
}

const OWNER_TRIGGER_NAME: &str = "BorrowedEngineOwnerPolygon";
const OWNER_UNIT_NAME: &str = "BorrowedEngineScout";
const OWNER_OBJECT_ID: u32 = 0x51_5E_0117;

#[cfg(not(target_arch = "wasm32"))]
fn isolated_owner_test(name: &str) -> bool {
    let module = module_path!();
    let prefix = concat!(env!("CARGO_CRATE_NAME"), "::");
    matches!(
        crate::test_process::run_bounded(
            &format!("{}::{name}", module.strip_prefix(prefix).unwrap_or(module)),
            "GENERALS_ENGINE_TRIGGER_OWNER_CHILD",
        ),
        crate::test_process::TestProcess::Child
    )
}

#[cfg(target_arch = "wasm32")]
fn isolated_owner_test(_: &str) -> bool {
    true
}

struct QuerySnapshotRestore(Option<crate::scripting::HostScriptQuerySnapshot>);
impl QuerySnapshotRestore {
    fn install() -> Self {
        let mut prior = None;
        crate::scripting::merge_host_script_query_snapshot(|snapshot| {
            prior = Some(std::mem::take(snapshot));
        });
        let guard = Self(prior);
        crate::scripting::set_host_script_query_snapshot(
            crate::scripting::HostScriptQuerySnapshot {
                named: [(OWNER_UNIT_NAME.to_string(), OWNER_OBJECT_ID)]
                    .into_iter()
                    .collect(),
                objects: vec![crate::scripting::HostScriptQueryObject {
                    id: OWNER_OBJECT_ID,
                    name: OWNER_UNIT_NAME.to_string(),
                    x: 2.0,
                    z: 2.0,
                    alive: true,
                    ..Default::default()
                }],
                // Foreign AABB deliberately agrees only with the near polygon.
                // The actual supplied owner must govern this named predicate.
                areas: [(OWNER_TRIGGER_NAME.to_string(), (0.0, 0.0, 20.0, 20.0))]
                    .into_iter()
                    .collect(),
                ..Default::default()
            },
        );
        guard
    }
}
impl Drop for QuerySnapshotRestore {
    fn drop(&mut self) {
        if let Some(prior) = self.0.take() {
            crate::scripting::set_host_script_query_snapshot(prior);
        }
    }
}

struct TerrainTriggersRestore(Vec<crate::polygon_trigger::PolygonTrigger>);
impl TerrainTriggersRestore {
    fn add(trigger: crate::polygon_trigger::PolygonTrigger) -> Self {
        let mut terrain = crate::terrain::get_terrain_logic().write().unwrap();
        let prior = terrain.get_trigger_areas().get_triggers().to_vec();
        terrain.get_trigger_areas_mut().add(trigger);
        Self(prior)
    }
}
impl Drop for TerrainTriggersRestore {
    fn drop(&mut self) {
        let mut terrain = crate::terrain::get_terrain_logic().write().unwrap();
        let triggers = terrain.get_trigger_areas_mut();
        triggers.clear();
        for trigger in self.0.drain(..) {
            triggers.add(trigger);
        }
    }
}

fn owner_polygon(offset: i32) -> crate::polygon_trigger::PolygonTrigger {
    crate::polygon_trigger::PolygonTrigger::new(
        0x117,
        crate::common::AsciiString::from(OWNER_TRIGGER_NAME),
        vec![
            crate::common::ICoord3D::new(offset, offset, 0),
            crate::common::ICoord3D::new(offset + 20, offset, 0),
            crate::common::ICoord3D::new(offset, offset + 20, 0),
        ],
    )
}

fn trigger_owner(offset: i32) -> Arc<Mutex<crate::scripting::HostTriggerWorld>> {
    let mut world = crate::scripting::HostTriggerWorld::default();
    world.set_trigger_areas(&[owner_polygon(offset)]);
    Arc::new(Mutex::new(world))
}

fn trigger_engine(kind: ConditionType) -> ScriptEngine {
    let mut condition = Condition::new(kind);
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            OWNER_UNIT_NAME.to_string(),
        ))
        .unwrap();
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::TriggerArea,
            OWNER_TRIGGER_NAME.to_string(),
        ))
        .unwrap();
    let mut or = OrCondition::new();
    or.set_first_and_condition(Some(Box::new(condition)));
    let mut script = Script::new();
    script.script_name = "BorrowedOwnerPredicate".to_string();
    // CPP Scripts.cpp:881 defaults to one-shot; these regressions require
    // recurring evaluation after geometry changes and frame advancement.
    script.is_one_shot = false;
    script.condition = Some(Box::new(or));
    script.action = Some(set_flag_action("owner_predicate_fired"));
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    let mut engine = ScriptEngine::new().unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    engine
}

fn update_with_trigger_owner(
    engine: &mut ScriptEngine,
    owner: &Arc<Mutex<crate::scripting::HostTriggerWorld>>,
    frame: u32,
) -> bool {
    // CPP ScriptEngine.cpp:5744–5758 clearFlag resets player-suffixed
    // symbols only. This observer is an exact unsuffixed script flag.
    engine.set_flag("owner_predicate_fired", false).unwrap();
    let mut context = ScriptContext::new();
    context.current_frame = frame;
    context.host_trigger_world = owner.clone();
    let mut driver = CanonicalScriptExecutionDriver;
    engine.update_with_driver(context, &mut driver).unwrap();
    engine
        .get_flag("owner_predicate_fired")
        .is_some_and(|flag| flag.value)
}

#[test]
fn regular_engine_named_inside_uses_interleaved_supplied_polygon_owners() {
    if !isolated_owner_test("regular_engine_named_inside_uses_interleaved_supplied_polygon_owners")
    {
        return;
    }
    let _guard = crate::test_sync::lock();
    assert!(
        OBJECT_REGISTRY.is_empty(),
        "fresh process must not admit unrelated objects"
    );
    let _query_restore = QuerySnapshotRestore::install();
    let near = trigger_owner(0);
    let far = trigger_owner(100);
    let mut first = trigger_engine(ConditionType::NamedInsideArea);
    let mut second = trigger_engine(ConditionType::NamedInsideArea);
    // CPP ScriptConditions.cpp:397–415 uses current integer position and
    // the resolved polygon, regardless of another world's identical ID/name.
    assert!(update_with_trigger_owner(&mut first, &near, 37));
    assert!(
        !update_with_trigger_owner(&mut second, &far, 37),
        "foreign AABB must not substitute for the far owner"
    );
    assert!(update_with_trigger_owner(&mut first, &near, 38));
    far.lock().unwrap().set_trigger_areas(&[owner_polygon(0)]);
    assert!(update_with_trigger_owner(&mut second, &far, 38));
    near.lock().unwrap().set_trigger_areas(&[]);
    assert!(!update_with_trigger_owner(&mut first, &near, 39));
    assert!(update_with_trigger_owner(&mut second, &far, 39));
}

#[test]
fn regular_engine_initialized_empty_polygon_owner_rejects_foreign_terrain() {
    if !isolated_owner_test(
        "regular_engine_initialized_empty_polygon_owner_rejects_foreign_terrain",
    ) {
        return;
    }
    let _guard = crate::test_sync::lock();
    assert!(OBJECT_REGISTRY.is_empty());
    let _query_restore = QuerySnapshotRestore::install();
    let _terrain_restore = TerrainTriggersRestore::add(owner_polygon(0));
    let empty = Arc::new(Mutex::new(crate::scripting::HostTriggerWorld::default()));
    empty.lock().unwrap().set_trigger_areas(&[]);
    let mut engine = trigger_engine(ConditionType::NamedInsideArea);
    assert!(
        !update_with_trigger_owner(&mut engine, &empty, 37),
        "initialized empty owner's missing polygon must not resolve through foreign TerrainLogic"
    );
}

#[test]
fn regular_engine_named_outside_inverts_resolved_supplied_owner_inside() {
    if !isolated_owner_test("regular_engine_named_outside_inverts_resolved_supplied_owner_inside") {
        return;
    }
    let _guard = crate::test_sync::lock();
    assert!(OBJECT_REGISTRY.is_empty());
    let _query_restore = QuerySnapshotRestore::install();
    let near = trigger_owner(0);
    let far = trigger_owner(100);
    let mut engine = trigger_engine(ConditionType::NamedOutsideArea);
    // CPP ScriptConditions.cpp:625–628: exact inversion of NamedInsideArea.
    assert!(!update_with_trigger_owner(&mut engine, &near, 37));
    assert!(update_with_trigger_owner(&mut engine, &far, 38));
    assert!(!update_with_trigger_owner(&mut engine, &near, 39));
}

#[test]
fn regular_engine_named_entered_uses_supplied_polygon_and_logic_frame_window() {
    if !isolated_owner_test(
        "regular_engine_named_entered_uses_supplied_polygon_and_logic_frame_window",
    ) {
        return;
    }
    let _guard = crate::test_sync::lock();
    assert!(OBJECT_REGISTRY.is_empty());
    let _query_restore = QuerySnapshotRestore::install();
    let near = trigger_owner(0);
    let far = trigger_owner(100);
    for owner in [&near, &far] {
        let mut world = owner.lock().unwrap();
        world.set_current_frame(36);
        world.update_object_flags(OWNER_OBJECT_ID, 30.0, 30.0, 35, false, None);
        world.update_object_flags(OWNER_OBJECT_ID, 2.0, 2.0, 36, false, None);
    }
    let mut first = trigger_engine(ConditionType::NamedEnteredArea);
    let mut second = trigger_engine(ConditionType::NamedEnteredArea);
    // ScriptConditions.cpp:1614–1631 resolves the polygon then Object::didEnter.
    // Object.cpp:2467–2500 accepts current/previous logic frame. Neither
    // the unpopulated ambient terrain nor its clock is this supplied owner.
    assert!(update_with_trigger_owner(&mut first, &near, 37));
    assert!(!update_with_trigger_owner(&mut second, &far, 37));
    assert!(!update_with_trigger_owner(&mut first, &near, 38));
}

#[test]
fn regular_engine_named_outside_missing_owned_polygon_is_inside_inversion() {
    if !isolated_owner_test(
        "regular_engine_named_outside_missing_owned_polygon_is_inside_inversion",
    ) {
        return;
    }
    let _guard = crate::test_sync::lock();
    assert!(OBJECT_REGISTRY.is_empty());
    let _query_restore = QuerySnapshotRestore::install();
    let empty = Arc::new(Mutex::new(crate::scripting::HostTriggerWorld::default()));
    empty.lock().unwrap().set_trigger_areas(&[]);
    let mut outside = trigger_engine(ConditionType::NamedOutsideArea);
    // CPP ScriptConditions.cpp:625–628 returns !NamedInside. A missing
    // polygon makes Inside false (411); Outside therefore returns true.
    assert!(update_with_trigger_owner(&mut outside, &empty, 37));
}

#[test]
fn regular_engine_uninitialized_standalone_context_preserves_terrain_fallback() {
    if !isolated_owner_test(
        "regular_engine_uninitialized_standalone_context_preserves_terrain_fallback",
    ) {
        return;
    }
    let _guard = crate::test_sync::lock();
    assert!(OBJECT_REGISTRY.is_empty());
    let _query_restore = QuerySnapshotRestore::install();
    let _terrain_restore = TerrainTriggersRestore::add(owner_polygon(0));
    let standalone = Arc::new(Mutex::new(crate::scripting::HostTriggerWorld::default()));
    assert!(!standalone.lock().unwrap().has_authored_trigger_geometry());
    let mut engine = trigger_engine(ConditionType::NamedInsideArea);
    assert!(update_with_trigger_owner(&mut engine, &standalone, 37));
}
