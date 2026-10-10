use super::*;
use crate::game_logic::{GameLogic, GridPos, KindOf, Team, ThingTemplate};
use glam::Vec3;
use std::io::Read;
use std::process::{Command, Stdio};

// Foreign stores are intentional negative controls, isolated without a new lock.
fn isolated(name: &str) -> bool {
    const MARKER: &str = "GENERALS_AI_DEFINITION_OWNER_TEST";
    let exact = format!("{}::{name}", module_path!().split_once("::").unwrap().1);
    if std::env::var(MARKER).as_deref() == Ok(exact.as_str()) {
        return false;
    }
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([&exact, "--exact", "--test-threads=1", "--nocapture"])
        .env(MARKER, &exact)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let pipes: [Box<dyn Read + Send>; 2] = [
        Box::new(child.stdout.take().unwrap()),
        Box::new(child.stderr.take().unwrap()),
    ];
    let readers = pipes.map(|mut pipe| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            pipe.read_to_end(&mut bytes).unwrap();
            bytes
        })
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(25);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if std::time::Instant::now() >= deadline {
            child.kill().unwrap();
            panic!("{exact} timed out");
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    };
    let output =
        readers.map(|reader| String::from_utf8_lossy(&reader.join().unwrap()).into_owned());
    assert!(status.success(), "{exact}: {output:?}");
    assert!(
        output[0].contains("1 passed; 0 failed"),
        "no exact witness: {output:?}"
    );
    true
}

fn authored_base(contents: &str) -> AIData {
    let target = Arc::new(RwLock::new(AIDataStore::default()));
    let mut ini = game_engine::common::ini::INI::new();
    ini.set_ai_data_store_target(Arc::clone(&target));
    ini.with_inline_source(contents, |ini| ini.parse_current_file())
        .unwrap();
    drop(ini);
    Arc::try_unwrap(target)
        .unwrap()
        .into_inner()
        .unwrap()
        .get_active()
        .unwrap()
        .clone()
}

fn world(contents: &str) -> (GameLogic, crate::game_logic::ObjectId) {
    let mut world = GameLogic::new();
    world.set_ai_definition_base(authored_base(contents));
    let mut template = ThingTemplate::new("OwnedDefinitionInfantry");
    template
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::AttackNeedsLineOfSight);
    world.templates.insert(template.name.clone(), template);
    let id = world
        .create_object("OwnedDefinitionInfantry", Team::USA, Vec3::ZERO)
        .unwrap();
    (world, id)
}

const BASE_A: &str = "AIData\n GuardEnemyScanRate = 500\n GuardEnemyReturnScanRate = 1000\n GuardChaseUnitsDuration = 2000\n AttackUsesLineOfSight = Yes\n WallHeight = 21\n RepulsedDistance = 40\n Poor = 1000\n TeamsPoorRate = 0.5\n End\n";
const BASE_B: &str = "AIData\n GuardEnemyScanRate = 1500\n GuardEnemyReturnScanRate = 2000\n GuardChaseUnitsDuration = 4000\n AttackUsesLineOfSight = No\n WallHeight = 55\n RepulsedDistance = 90\n Poor = 7000\n TeamsPoorRate = 2\n End\n";

fn blocked(world: &GameLogic) -> bool {
    world
        .pathfinding_system
        .is_attack_view_blocked(Vec3::new(-200.0, 0.0, 0.0), Vec3::new(200.0, 0.0, 0.0))
}

fn barrier(world: &mut GameLogic) {
    let x = world.pathfinding_system.grid.width() / 2;
    for y in 0..world.pathfinding_system.grid.height() {
        world.pathfinding_system.grid.set_cell_obstacle_owned(
            GridPos::new(x, y),
            false,
            false,
            999,
            Some(2),
            Some(Team::GLA),
        );
    }
}

#[test]
fn same_id_world_rules_ignore_foreign_common_and_held_core_ai() {
    if isolated("same_id_world_rules_ignore_foreign_common_and_held_core_ai") {
        return;
    }
    let (mut a, a_id) = world(BASE_A);
    let (mut b, b_id) = world(BASE_B);
    assert_eq!(a_id, b_id);
    barrier(&mut a);
    barrier(&mut b);
    let foreign = gamelogic::system::engine_stores::new_for_world();
    {
        let mut common = foreign.ai_data().write().unwrap();
        common.ensure_base();
        common.get_active_mut().unwrap().guard_enemy_scan_rate = 777;
        common.get_active_mut().unwrap().attack_uses_line_of_sight = false;
    }
    let engine_base = AiDefinitions::from_engine_baseline();
    let core_guard = foreign.ai().write().unwrap();
    gamelogic::system::engine_stores::with_active_stores(&foreign, || {
        assert_eq!(a.host_guard_enemy_scan_rate(), 15);
        assert_eq!(b.host_guard_enemy_scan_rate(), 45);
        assert_eq!(a.host_guard_chase_unit_frames(), 60);
        assert_eq!(b.host_guard_chase_unit_frames(), 120);
        assert!(blocked(&a));
        assert!(!blocked(&b));
        assert_eq!(a.ai_definitions.data().wall_height, 21.0);
        assert_eq!(b.ai_definitions.data().repulsed_distance, 90.0);
        let before = Arc::clone(&a.ai_definitions.snapshot());
        b.reset();
        b.override_world_size(600.0, 600.0);
        barrier(&mut b);
        assert!(Arc::ptr_eq(&before, &a.ai_definitions.snapshot()));
        assert_eq!(a.host_guard_enemy_scan_rate(), 15);
        assert!(!blocked(&b));
        let candidate = GameLogic::new();
        assert_eq!(
            format!("{:?}", candidate.ai_definitions.baseline()),
            format!("{:?}", engine_base.baseline()),
            "constructor samples engine content, never a selected foreign world"
        );
        assert!(Arc::ptr_eq(
            &gamelogic::system::engine_stores::active(),
            &foreign
        ));
        drop(candidate);
        drop(b);
        assert_eq!(a.host_guard_enemy_scan_rate(), 15);
        assert!(blocked(&a));
    });
    drop(core_guard);
}

struct MapFixture(std::path::PathBuf);
impl MapFixture {
    fn new(label: &str, map_ini: &str, solo_ini: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "generals-ai-definitions-{}-{label}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let mut output = game_engine::common::system::DataChunkOutput::new();
        output.open_data_chunk("OwnerWitness", 1);
        output.write_int(0);
        output.close_data_chunk();
        std::fs::write(dir.join("Owner.map"), output.into_ckmp_bytes()).unwrap();
        std::fs::write(dir.join("Map.ini"), map_ini).unwrap();
        std::fs::write(dir.join("Solo.ini"), solo_ini).unwrap();
        Self(dir)
    }
    fn admit(&self, world: &mut GameLogic) {
        let path = self.0.join("Owner.map");
        let document = crate::game_logic::script_loader::load_chunky_map(path.to_str().unwrap())
            .unwrap()
            .unwrap();
        let draft = world.ai_definitions.map_override_draft();
        crate::game_logic::script_loader::parse_map_settings_from_chunky_with_ai_data(
            &document,
            Arc::clone(&draft),
        )
        .unwrap();
        world.admit_ai_map_overrides(draft).unwrap();
    }
}
impl Drop for MapFixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn authored_map_then_solo_override_only_their_world_and_reset_to_base() {
    if isolated("authored_map_then_solo_override_only_their_world_and_reset_to_base") {
        return;
    }
    let (mut a, _) = world(BASE_A);
    let (mut b, _) = world(BASE_B);
    let a_map = MapFixture::new(
        "a",
        "AIData\n GuardEnemyScanRate = 2500\n WallHeight = 31\n End\n",
        "AIData\n GuardEnemyScanRate = 3500\n AttackUsesLineOfSight = No\n End\n",
    );
    let b_map = MapFixture::new(
        "b",
        "AIData\n WallHeight = 65\n End\n",
        "AIData\n AttackUsesLineOfSight = Yes\n End\n",
    );
    let foreign = gamelogic::system::engine_stores::new_for_world();
    {
        let mut catalog = foreign.ai_data().write().unwrap();
        catalog.ensure_base();
        catalog.get_active_mut().unwrap().wall_height = 999.0;
    }
    gamelogic::system::engine_stores::with_active_stores(&foreign, || {
        a_map.admit(&mut a);
        b_map.admit(&mut b);
        assert_eq!(a.host_guard_enemy_scan_rate(), 105);
        assert_eq!(a.ai_definitions.data().wall_height, 31.0);
        assert_eq!(a.ai_definitions.data().repulsed_distance, 40.0);
        assert_eq!(b.ai_definitions.data().wall_height, 65.0);
        assert_eq!(b.host_guard_enemy_scan_rate(), 45);
        assert_eq!(a.ai_definitions.layers.len(), 3);
        assert_eq!(b.ai_definitions.layers.len(), 3);
        assert_eq!(
            foreign
                .ai_data()
                .read()
                .unwrap()
                .get_active()
                .unwrap()
                .wall_height,
            999.0
        );
        barrier(&mut a);
        barrier(&mut b);
        assert!(!blocked(&a));
        assert!(blocked(&b));
        b.reset();
        assert_eq!(b.ai_definitions.layers.len(), 1);
        assert_eq!(b.ai_definitions.data().wall_height, 55.0);
        assert_eq!(a.host_guard_enemy_scan_rate(), 105);
        a.reset();
        barrier(&mut a);
        assert_eq!(a.host_guard_enemy_scan_rate(), 15);
        assert!(blocked(&a));
    });
}

#[test]
fn failed_definition_draft_and_candidate_drop_preserve_live_snapshot() {
    if isolated("failed_definition_draft_and_candidate_drop_preserve_live_snapshot") {
        return;
    }
    let (mut live, _) = world(BASE_A);
    let map = MapFixture::new(
        "live",
        "AIData\n WallHeight = 31\n End\n",
        "AIData\n GuardEnemyScanRate = 3500\n End\n",
    );
    map.admit(&mut live);
    let retained = live.ai_definitions.snapshot();
    let mut candidate = GameLogic::new();
    candidate.set_ai_definition_base(live.ai_definitions.baseline().clone());
    assert_eq!(
        candidate.ai_definitions.data().wall_height,
        21.0,
        "restore starts from engine baseline, then rebuilds map overrides"
    );
    let draft = candidate.ai_definitions.map_override_draft();
    assert!(
        gamelogic::system::load_map_ini_ui_overrides_with_ai_data(
            "AIData\n WallHeight = 777\n UnknownOwnerField = 1\n End\n",
            Arc::clone(&draft)
        )
        .is_err()
    );
    drop(draft);
    drop(candidate);
    assert!(Arc::ptr_eq(&retained, &live.ai_definitions.snapshot()));
    assert_eq!(live.ai_definitions.data().wall_height, 31.0);
    assert_eq!(live.host_guard_enemy_scan_rate(), 105);
    let mut restored = GameLogic::new();
    restored.set_ai_definition_base(live.ai_definitions.baseline().clone());
    map.admit(&mut restored);
    assert_eq!(
        format!("{:?}", restored.ai_definitions.data()),
        format!("{:?}", live.ai_definitions.data())
    );
    assert!(
        Arc::ptr_eq(&retained, &live.ai_definitions.snapshot()),
        "successful candidate reconstruction does not replace live definitions"
    );
}
