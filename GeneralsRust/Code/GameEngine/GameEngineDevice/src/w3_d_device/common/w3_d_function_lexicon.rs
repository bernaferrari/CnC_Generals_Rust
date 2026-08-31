// FILE: w3_d_function_lexicon.rs
// Ported from C++ W3DFunctionLexicon.h and FunctionLexicon.h

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Mutex, OnceLock};

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

static NAME_KEY_STATE: OnceLock<Mutex<NameKeyGeneratorState>> = OnceLock::new();

fn with_name_key_state<T>(f: impl FnOnce(&mut NameKeyGeneratorState) -> T) -> T {
    let state = NAME_KEY_STATE.get_or_init(|| Mutex::new(NameKeyGeneratorState::new()));
    let mut guard = state.lock().expect("NameKeyGenerator mutex poisoned");
    f(&mut guard)
}

fn name_to_key(name: &str) -> NameKeyType {
    with_name_key_state(|state| state.name_to_key(name))
}

/// Function pointer stored in the W3D lexicon tables.
///
/// Stores a real `fn()` (or none). Callers construct via [`FunctionPtr::from_fn`]
/// so the value cannot be forged from an arbitrary integer and later
/// `transmute`d. `fn()` is already `Send + Sync` — no `unsafe impl`.
///
/// `Eq`/`Hash` use the code address so duplicate-registration checks can
/// compare entries the same way the previous `*const ()` wrapper did.
#[derive(Debug, Clone, Copy)]
pub struct FunctionPtr(Option<fn()>);

impl PartialEq for FunctionPtr {
    fn eq(&self, other: &Self) -> bool {
        self.as_ptr() == other.as_ptr()
    }
}

impl Eq for FunctionPtr {}

impl Hash for FunctionPtr {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (self.as_ptr() as usize).hash(state);
    }
}

impl FunctionPtr {
    /// Wrap a typed unit function pointer.
    pub fn from_fn(f: fn()) -> Self {
        Self(Some(f))
    }

    /// Reconstruct from a raw address (C++ FunctionLexicon static tables).
    ///
    /// # Safety
    ///
    /// `raw` must be `0` (null) or the address of a live `fn()` that remains
    /// valid for the rest of the process (typical for C++ static callback
    /// tables and Rust function items). Passing a non-function address is
    /// undefined behavior if the result is later invoked via
    /// [`FunctionPtr::as_unit_fn`].
    // SAFETY: Caller must pass 0 or the address of a live `fn()` that stays valid
    // SAFETY: for process lifetime (C++ static callback tables / Rust function
    // SAFETY: items). Any other address is UB if later invoked via as_unit_fn.
    pub unsafe fn from_usize(raw: usize) -> Self {
        if raw == 0 {
            Self(None)
        } else {
            // Safety: caller guarantees `raw` is a valid `fn()` address or 0.
            Self(Some(std::mem::transmute::<usize, fn()>(raw)))
        }
    }

    /// Convert back to a raw function pointer address (null if unset).
    pub fn as_ptr(self) -> *const () {
        self.0.map(|f| f as *const ()).unwrap_or(std::ptr::null())
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
        self.0
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
#[derive(Debug, Default)]
pub struct FunctionLexicon {
    tables: Vec<Vec<TableEntry>>,
}

impl FunctionLexicon {
    #[must_use]
    pub fn new() -> Self {
        Self {
            tables: vec![Vec::new(); TableIndex::MaxFunctionTables as usize],
        }
    }

    pub fn init(&mut self) {
        with_name_key_state(NameKeyGeneratorState::reset);
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
                entry.key = name_to_key(entry.name);
            }
        }
        let index = table_index.as_usize();
        if index < self.tables.len() {
            self.tables[index] = table;
        }
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

    #[must_use]
    pub fn find_function(&self, key: NameKeyType, index: TableIndex) -> Option<FunctionPtr> {
        self.base.find_function(key, index)
    }

    #[must_use]
    pub fn get_table(&self, index: TableIndex) -> Option<&[TableEntry]> {
        self.base.get_table(index)
    }
}
