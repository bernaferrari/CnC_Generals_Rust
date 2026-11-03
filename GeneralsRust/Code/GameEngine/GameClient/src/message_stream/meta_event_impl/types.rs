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
pub(crate) struct MetaMapRec {
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
pub struct MetaMap {
    pub(crate) records: Vec<MetaMapRec>,
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

    pub(crate) fn iter(&self) -> impl Iterator<Item = &MetaMapRec> {
        self.records.iter()
    }
}

// Parser registration is a process-wide one-shot (idempotent). The CommandMap
// table itself lives on `GameClient::meta_map` (live client / test TLS).
static META_PARSER_REGISTERED: OnceLock<()> = OnceLock::new();

const DROPPED_MAX_PARTICLE_COUNT: i32 = 1000;
const EXTENT_BIG_CHANGE: f32 = 10.0;
const DEMO_CAMERA_ADJUST_FACTOR: f32 = 0.01;

use crate::meta_demo_state::{DemoCameraAdjustState, LowerDetailToggleState, MetaDemoState};

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

