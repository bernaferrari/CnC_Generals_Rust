//! Named attack-priority sets and per-object assignments in one simulation.
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

pub const MAX_ATTACK_PRIORITIES: usize = 256;

/// Attack Priority Info matching C++ AttackPriorityInfo
#[derive(Debug, Clone)]
pub struct AttackPriorityInfo {
    pub name: String,
    pub default_priority: i32,
    pub priority_map: HashMap<String, i32>, // ThingTemplate name -> priority
}

impl Default for AttackPriorityInfo {
    fn default() -> Self {
        Self::new()
    }
}

impl AttackPriorityInfo {
    pub fn new() -> Self {
        Self::named(String::new())
    }

    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            default_priority: 1,
            priority_map: HashMap::new(),
        }
    }

    pub fn set_priority(&mut self, thing_template: &str, priority: i32) {
        self.priority_map
            .insert(thing_template.to_string(), priority);
    }

    pub fn get_priority(&self, thing_template: &str) -> i32 {
        self.priority_map
            .get(thing_template)
            .copied()
            .unwrap_or(self.default_priority)
    }

    /// Main's template names may still contain aliases with different casing.
    pub fn get_priority_for_template(&self, name: &str) -> i32 {
        self.priority_map
            .get(name)
            .or_else(|| {
                self.priority_map
                    .iter()
                    .filter(|(key, _)| key.eq_ignore_ascii_case(name))
                    .min_by_key(|(key, _)| *key)
                    .map(|(_, value)| value)
            })
            .copied()
            .unwrap_or(self.default_priority)
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }
}

pub struct AttackPriorityTable {
    rows: Vec<Arc<AttackPriorityInfo>>,
    object_sets: BTreeMap<u32, String>,
}

impl Default for AttackPriorityTable {
    fn default() -> Self {
        Self::new()
    }
}

impl AttackPriorityTable {
    pub fn new() -> Self {
        Self {
            rows: Vec::with_capacity(MAX_ATTACK_PRIORITIES),
            object_sets: BTreeMap::new(),
        }
    }

    pub fn ensure_default(&mut self) {
        if self.rows.is_empty() {
            self.rows.push(Arc::new(AttackPriorityInfo::new()));
        }
    }

    pub fn clear(&mut self) {
        self.rows.clear();
        self.object_sets.clear();
    }

    /// The original interpreter searches named rows before falling back to zero.
    pub fn get(&self, name: &str) -> Option<&Arc<AttackPriorityInfo>> {
        self.rows
            .iter()
            .skip(1)
            .find(|info| info.name == name)
            .or_else(|| self.rows.first())
    }

    pub fn with_mut<R>(
        &mut self,
        name: &str,
        add_if_missing: bool,
        f: impl FnOnce(&mut AttackPriorityInfo) -> R,
    ) -> Option<R> {
        self.ensure_default();
        if let Some(index) = self.rows.iter().skip(1).position(|info| info.name == name) {
            return Some(f(Arc::make_mut(&mut self.rows[index + 1])));
        }

        if add_if_missing && self.rows.len() < MAX_ATTACK_PRIORITIES {
            let index = self.rows.len();
            self.rows.push(Arc::new(AttackPriorityInfo::named(name)));
            return Some(f(Arc::make_mut(&mut self.rows[index])));
        }

        None
    }

    pub fn entries(&self) -> &[Arc<AttackPriorityInfo>] {
        &self.rows
    }

    /// The legacy codec keeps its wire format while the row data lives here.
    pub fn legacy_rows_mut(&mut self) -> &mut [Arc<AttackPriorityInfo>] {
        &mut self.rows
    }

    pub fn restore(&mut self, rows: Vec<AttackPriorityInfo>) {
        self.rows = rows.into_iter().map(Arc::new).collect();
        self.ensure_default();
    }

    pub fn assigned_set(&self, object: u32) -> Option<&str> {
        self.object_sets.get(&object).map(String::as_str)
    }

    pub fn assign(&mut self, object: u32, set: &str) {
        // Zero is the original invalid object ID.
        if object == 0 {
            return;
        }
        if set.is_empty() {
            self.remove_assignment(object);
        } else {
            self.object_sets.insert(object, set.to_owned());
        }
    }

    pub fn remove_assignment(&mut self, object: u32) {
        self.object_sets.remove(&object);
    }

    pub fn clear_assignments(&mut self) {
        self.object_sets.clear();
    }

    pub fn assignments(&self) -> impl ExactSizeIterator<Item = (u32, &str)> {
        self.object_sets
            .iter()
            .map(|(&object, set)| (object, set.as_str()))
    }
}
