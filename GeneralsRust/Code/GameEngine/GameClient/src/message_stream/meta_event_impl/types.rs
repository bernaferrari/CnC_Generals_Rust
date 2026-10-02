// Split from `message_stream/meta_event.rs` dump. Included by `meta_event_impl/mod.rs`.

const MOD_CTRL: u32 = 1;
const MOD_ALT: u32 = 2;
const MOD_SHIFT: u32 = 4;

const KEY_STATE_CONTROL: u32 = 0x0004 | 0x0008;
const KEY_STATE_SHIFT: u32 = 0x0010 | 0x0020 | 0x0400;
const KEY_STATE_ALT: u32 = 0x0040 | 0x0080;
const KEY_STATE_DOWN: u32 = 0x0002;
const KEY_STATE_UP: u32 = 0x0001;
const KEY_STATE_AUTOREPEAT: u32 = 0x0100;

const COMMANDUSABLE_NONE: u32 = 0;
const COMMANDUSABLE_SHELL: u32 = 1 << 0;
const COMMANDUSABLE_GAME: u32 = 1 << 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Transition {
    Down,
    Up,
    DoubleDown,
}

#[derive(Debug, Clone)]
struct MetaMapRec {
    name: String,
    meta: Option<GameMessageType>,
    key: u32,
    transition: Transition,
    mod_state: u32,
    usable_in: u32,
    category: String,
    description: String,
    display_name: String,
}

#[derive(Debug, Clone)]
pub struct CommandMapEntry {
    pub name: String,
    pub key: u32,
    pub mod_state: u32,
    pub category: String,
    pub description: String,
    pub display_name: String,
}

#[derive(Default)]
struct MetaMap {
    records: Vec<MetaMapRec>,
}

impl MetaMap {
    fn add_record(&mut self, record: MetaMapRec) {
        let existing_index = self.records.iter().position(|existing| {
            if let (Some(existing_meta), Some(new_meta)) = (&existing.meta, &record.meta) {
                return existing_meta == new_meta;
            }
            existing.meta.is_none()
                && record.meta.is_none()
                && existing.name.eq_ignore_ascii_case(&record.name)
        });

        if let Some(index) = existing_index {
            self.records[index] = record;
        } else {
            self.records.push(record);
        }
    }

    fn iter(&self) -> impl Iterator<Item = &MetaMapRec> {
        self.records.iter()
    }
}

// THREAD: producer -> consumer across the host's startup worker. The host parses
// CommandMap.ini on a background startup thread (`Main::spawn_startup_map_load`), and
// the registered "CommandMap" INI block parser writes straight into this table, so
// the map itself has to stay a process-wide lock; the meta-event toggles/cheat state
// that is only ever touched by GUI-thread handlers lives in the thread-local cell
// block in `state.rs`.
static META_MAP: OnceLock<RwLock<MetaMap>> = OnceLock::new();
static META_PARSER_REGISTERED: OnceLock<()> = OnceLock::new();

const DROPPED_MAX_PARTICLE_COUNT: i32 = 1000;
const EXTENT_BIG_CHANGE: f32 = 10.0;
const DEMO_CAMERA_ADJUST_FACTOR: f32 = 0.01;

#[derive(Debug, Clone)]
struct LowerDetailToggleState {
    is_low_details: bool,
    old_use_shadow_volumes: bool,
    old_use_light_map: bool,
    old_use_cloud_map: bool,
    old_show_behind_building_markers: bool,
    old_max_particle_count: i32,
}

#[derive(Debug, Clone, Default)]
struct DemoCameraAdjustState {
    is_pitching: bool,
    is_changing_fov: bool,
    anchor: ICoord2D,
    current_pos: ICoord2D,
}

#[derive(Debug, Clone, Copy)]
enum ExtentAdjustAxis {
    Type,
    Major,
    Minor,
    Height,
}

#[derive(Debug, Clone, Copy)]
struct ExtentAdjustSpec {
    axis: ExtentAdjustAxis,
    amount: f32,
}

impl Default for LowerDetailToggleState {
    fn default() -> Self {
        Self {
            is_low_details: false,
            old_use_shadow_volumes: true,
            old_use_light_map: true,
            old_use_cloud_map: true,
            old_show_behind_building_markers: true,
            old_max_particle_count: 5000,
        }
    }
}
