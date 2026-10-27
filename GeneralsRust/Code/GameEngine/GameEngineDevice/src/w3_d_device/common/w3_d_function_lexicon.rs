// FILE: w3_d_function_lexicon.rs
// Ported from C++ W3DFunctionLexicon.h and FunctionLexicon.h

use game_client::gui::GameWindow;
use game_client::gui::game_window::WindowInstanceData;
use game_client::gui::window_draw_context::WindowDrawContext;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

/// Name key type (matches C++ `NameKeyType`)
pub type NameKeyType = u32;

/// Invalid name key value (matches C++ `NAMEKEY_INVALID`)
pub const NAMEKEY_INVALID: NameKeyType = 0;

/// Maximum name key value (matches C++ `NAMEKEY_MAX`)
pub const NAMEKEY_MAX: NameKeyType = 1 << 23;

const SOCKET_COUNT: usize = 45_007;

#[derive(Debug, Clone)]
struct BucketEntry {
    key: NameKeyType,
    name: String,
}

#[derive(Debug)]
struct NameKeyGeneratorState {
    buckets: Vec<Vec<BucketEntry>>,
    next_id: NameKeyType,
    reverse_lookup: HashMap<NameKeyType, String>,
}

impl NameKeyGeneratorState {
    fn new() -> Self {
        let mut buckets = Vec::with_capacity(SOCKET_COUNT);
        buckets.resize_with(SOCKET_COUNT, Vec::new);
        Self {
            buckets,
            next_id: 1,
            reverse_lookup: HashMap::new(),
        }
    }

    fn reset(&mut self) {
        for bucket in &mut self.buckets {
            bucket.clear();
        }
        self.next_id = 1;
        self.reverse_lookup.clear();
    }

    fn allocate_key(&mut self, name: &str) -> NameKeyType {
        let key = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        self.reverse_lookup.insert(key, name.to_string());
        key
    }

    fn name_to_key(&mut self, name: &str) -> NameKeyType {
        let index = calc_hash(name, false);
        if let Some(entry) = self.buckets[index].iter().find(|entry| entry.name == name) {
            return entry.key;
        }
        let key = self.allocate_key(name);
        self.buckets[index].push(BucketEntry {
            key,
            name: name.to_string(),
        });
        key
    }
}

/// WND draws borrow the receiving client's services for one synchronous call.
pub type WindowDrawFn = fn(&GameWindow, &WindowInstanceData, &mut WindowDrawContext<'_>);

#[derive(Debug, Clone, Copy)]
enum FunctionKind {
    Unit(fn()),
    WindowDraw(WindowDrawFn),
}

/// A lexicon entry retains its callback's actual signature.
#[derive(Debug, Clone, Copy)]
pub struct FunctionPtr(Option<FunctionKind>);

impl PartialEq for FunctionPtr {
    fn eq(&self, other: &Self) -> bool {
        self.kind_id() == other.kind_id() && self.as_ptr() == other.as_ptr()
    }
}

impl Eq for FunctionPtr {}

impl Hash for FunctionPtr {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.kind_id().hash(state);
        (self.as_ptr() as usize).hash(state);
    }
}

impl FunctionPtr {
    fn kind_id(self) -> u8 {
        match self.0 {
            None => 0,
            Some(FunctionKind::Unit(_)) => 1,
            Some(FunctionKind::WindowDraw(_)) => 2,
        }
    }

    /// Wrap a typed unit function pointer.
    pub fn from_fn(f: fn()) -> Self {
        Self(Some(FunctionKind::Unit(f)))
    }

    /// Wrap a draw function without erasing its borrowed context.
    pub fn from_window_draw(f: WindowDrawFn) -> Self {
        Self(Some(FunctionKind::WindowDraw(f)))
    }

    /// Convert back to a raw function pointer address (null if unset).
    pub fn as_ptr(self) -> *const () {
        match self.0 {
            Some(FunctionKind::Unit(f)) => f as *const (),
            Some(FunctionKind::WindowDraw(f)) => f as *const (),
            None => std::ptr::null(),
        }
    }

    /// Create a null function pointer.
    pub fn null() -> Self {
        Self(None)
    }

    /// Check if the function pointer is null.
    pub fn is_null(self) -> bool {
        self.0.is_none()
    }

    /// Convert to a typed unit function pointer (`fn()`).
    pub fn as_unit_fn(self) -> Option<fn()> {
        match self.0 {
            Some(FunctionKind::Unit(f)) => Some(f),
            _ => None,
        }
    }

    /// Retrieve a window draw only from a matching table entry.
    pub fn as_window_draw(self) -> Option<WindowDrawFn> {
        match self.0 {
            Some(FunctionKind::WindowDraw(f)) => Some(f),
            _ => None,
        }
    }
}

/// Function table entry (matches C++ `FunctionLexicon::TableEntry`)
#[derive(Debug, Clone)]
pub struct TableEntry {
    pub key: NameKeyType,
    pub name: &'static str,
    pub func: Option<FunctionPtr>,
}

impl TableEntry {
    #[must_use]
    pub fn new(name: &'static str, func: Option<FunctionPtr>) -> Self {
        Self {
            key: NAMEKEY_INVALID,
            name,
            func,
        }
    }
}

/// Table indices (matches C++ `FunctionLexicon::TableIndex`)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum TableIndex {
    Any = -1,
    GameWinSystem = 0,
    GameWinInput = 1,
    GameWinTooltip = 2,
    GameWinDeviceDraw = 3,
    GameWinDraw = 4,
    WinLayoutInit = 5,
    WinLayoutDeviceInit = 6,
    WinLayoutUpdate = 7,
    WinLayoutShutdown = 8,
    MaxFunctionTables = 9,
}

impl TableIndex {
    fn as_usize(self) -> usize {
        self as usize
    }
}

/// Function lexicon for managing function pointer lookups by name.
#[derive(Debug)]
pub struct FunctionLexicon {
    tables: Vec<Vec<TableEntry>>,
    names: NameKeyGeneratorState,
}

impl Default for FunctionLexicon {
    fn default() -> Self {
        Self::new()
    }
}

impl FunctionLexicon {
    #[must_use]
    pub fn new() -> Self {
        Self {
            tables: vec![Vec::new(); TableIndex::MaxFunctionTables as usize],
            names: NameKeyGeneratorState::new(),
        }
    }

    pub fn init(&mut self) {
        self.names.reset();
    }

    pub fn reset(&mut self) {
        for table in &mut self.tables {
            table.clear();
        }
    }

    pub fn update(&mut self) {}

    pub fn load_table(&mut self, mut table: Vec<TableEntry>, table_index: TableIndex) {
        for entry in &mut table {
            if !entry.name.is_empty() {
                entry.key = self.names.name_to_key(entry.name);
            }
        }
        let index = table_index.as_usize();
        if index < self.tables.len() {
            self.tables[index] = table;
        }
    }

    /// Resolve names in the same lexicon that will receive the lookup.
    pub fn key_for_name(&mut self, name: &str) -> NameKeyType {
        self.names.name_to_key(name)
    }

    pub fn find_function_by_name(&mut self, name: &str, index: TableIndex) -> Option<FunctionPtr> {
        let key = self.key_for_name(name);
        self.find_function(key, index)
    }

    #[must_use]
    pub fn find_function(&self, key: NameKeyType, index: TableIndex) -> Option<FunctionPtr> {
        if key == NAMEKEY_INVALID {
            return None;
        }
        if index == TableIndex::Any {
            for table in &self.tables {
                if let Some(func) = key_to_func(key, table) {
                    return Some(func);
                }
            }
            None
        } else {
            let idx = index.as_usize();
            self.tables
                .get(idx)
                .and_then(|table| key_to_func(key, table))
        }
    }

    #[must_use]
    pub fn get_table(&self, index: TableIndex) -> Option<&[TableEntry]> {
        let idx = index.as_usize();
        self.tables.get(idx).map(std::vec::Vec::as_slice)
    }
}

fn key_to_func(key: NameKeyType, table: &[TableEntry]) -> Option<FunctionPtr> {
    table
        .iter()
        .find(|entry| entry.key == key)
        .and_then(|entry| entry.func)
}

fn calc_hash(name: &str, lowercase: bool) -> usize {
    let mut result: u32 = 0;
    for byte in name.bytes() {
        let b = if lowercase {
            byte.to_ascii_lowercase()
        } else {
            byte
        };
        result = result.wrapping_mul(33).wrapping_add(u32::from(b));
    }
    (result as usize) % SOCKET_COUNT
}

/// W3D function lexicon (device-specific extension of `FunctionLexicon`).
#[derive(Debug, Default)]
pub struct W3DFunctionLexicon {
    base: FunctionLexicon,
}

impl W3DFunctionLexicon {
    #[must_use]
    pub fn new() -> Self {
        Self {
            base: FunctionLexicon::new(),
        }
    }

    pub fn init(&mut self) {
        self.base.init();
        crate::w3_d_device::common::system::w3_d_function_lexicon::load_w3d_tables(&mut self.base);
    }

    pub fn reset(&mut self) {
        self.base.reset();
    }

    pub fn update(&mut self) {
        self.base.update();
    }

    pub fn key_for_name(&mut self, name: &str) -> NameKeyType {
        self.base.key_for_name(name)
    }

    pub fn find_function_by_name(&mut self, name: &str, index: TableIndex) -> Option<FunctionPtr> {
        self.base.find_function_by_name(name, index)
    }

    #[must_use]
    pub fn find_function(&self, key: NameKeyType, index: TableIndex) -> Option<FunctionPtr> {
        self.base.find_function(key, index)
    }

    #[must_use]
    pub fn get_table(&self, index: TableIndex) -> Option<&[TableEntry]> {
        self.base.get_table(index)
    }
}
