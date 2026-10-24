//! ScriptEngine.cpp5975–6003,7068–7168,7693: one ordered named request vector.
//! Runtime targets retain the existing Object birth identity, never a second ID ledger.
use super::ObjectId;
use std::sync::{Arc, Weak};
#[derive(Clone, Debug)]
pub(crate) struct NamedScriptTarget {
    pub id: ObjectId,
    birth: Weak<()>,
}
impl NamedScriptTarget {
    pub fn matches(&self, id: ObjectId, birth: &Arc<()>) -> bool {
        self.id == id && self.birth.ptr_eq(&Arc::downgrade(birth))
    }
}
#[derive(Clone, Debug)]
struct NamedRequest {
    name: String,
    target: Option<NamedScriptTarget>,
}
#[derive(Clone, Debug, Default)]
pub(crate) struct NamedScriptCache {
    entries: Vec<NamedRequest>,
}
impl NamedScriptCache {
    /// Initial traversal appends every named actor, including duplicate names.
    pub fn seed(&mut self, objects: impl IntoIterator<Item = (String, ObjectId, Arc<()>)>) {
        self.entries = objects
            .into_iter()
            .filter(|(name, _, _)| !name.is_empty())
            .map(|(name, id, birth)| NamedRequest {
                name,
                target: Some(NamedScriptTarget {
                    id,
                    birth: Arc::downgrade(&birth),
                }),
            })
            .collect();
    }
    /// The name comparison precedes pointer comparison in each original visit.
    pub fn add(&mut self, name: &str, id: ObjectId, birth: &Arc<()>) -> bool {
        if name.is_empty() {
            return false;
        }
        for row in &mut self.entries {
            if row.name == name {
                if row.target.is_none() {
                    row.target = Some(NamedScriptTarget {
                        id,
                        birth: Arc::downgrade(birth),
                    });
                    return true;
                }
                return false;
            }
            if row
                .target
                .as_ref()
                .is_some_and(|target| target.matches(id, birth))
            {
                row.name = name.into();
                return true;
            }
        }
        self.entries.push(NamedRequest {
            name: name.into(),
            target: Some(NamedScriptTarget {
                id,
                birth: Arc::downgrade(birth),
            }),
        });
        true
    }
    /// Physical destruction and transfer's target-old-name removal null one row.
    pub fn remove(&mut self, id: ObjectId, birth: &Arc<()>) {
        if let Some(row) = self.entries.iter_mut().find(|row| {
            row.target
                .as_ref()
                .is_some_and(|target| target.matches(id, birth))
        }) {
            row.target = None;
        }
    }
    /// Caller sets target.name even when no cache row matches. Donor name remains.
    /// Return the previous target receipt for the source custom-indicator transfer.
    pub fn transfer(
        &mut self,
        name: &str,
        id: ObjectId,
        birth: &Arc<()>,
        target_named: bool,
    ) -> Option<NamedScriptTarget> {
        if target_named {
            self.remove(id, birth);
        }
        let row = self.entries.iter_mut().find(|row| row.name == name)?;
        row.target.replace(NamedScriptTarget {
            id,
            birth: Arc::downgrade(birth),
        })
    }
    pub fn lookup(&self, name: &str) -> Option<&NamedScriptTarget> {
        self.entries
            .iter()
            .find(|row| row.name == name)
            .and_then(|row| row.target.as_ref())
    }
    /// CPP didUnitExist means the FIRST matching cache row is NULL.
    pub fn did_exist(&self, name: &str) -> bool {
        self.entries
            .iter()
            .find(|row| row.name == name)
            .is_some_and(|row| row.target.is_none())
    }
    pub fn clear(&mut self) {
        self.entries.clear();
    }
    pub fn iter(&self) -> impl Iterator<Item = &String> {
        self.entries.iter().map(|row| &row.name)
    }
    #[cfg(test)]
    pub fn insert(&mut self, name: String) -> bool {
        if self.entries.iter().any(|row| row.name == name) {
            return false;
        }
        self.entries.push(NamedRequest { name, target: None });
        true
    }
    #[cfg(test)]
    pub fn contains(&self, name: &str) -> bool {
        self.entries.iter().any(|row| row.name == name)
    }
    pub fn wire(&self) -> Vec<(String, u32)> {
        self.entries
            .iter()
            .map(|row| {
                (
                    row.name.clone(),
                    row.target.as_ref().map_or(0, |target| target.id.0),
                )
            })
            .collect()
    }
    /// Validate and bind ALL nonzero IDs before replacing the running vector.
    /// Duplicate names and NULL rows are source-valid history, not malformed maps.
    pub fn from_wire(
        rows: &[(String, u32)],
        mut birth: impl FnMut(ObjectId) -> Option<Arc<()>>,
    ) -> Result<Self, String> {
        if rows.len() > u16::MAX as usize {
            return Err("named cache exceeds original u16 count".into());
        }
        let mut entries = Vec::with_capacity(rows.len());
        for (name, raw) in rows {
            if name.contains('\0') {
                return Err("invalid named cache name".into());
            }
            let target = if *raw == 0 {
                None
            } else {
                let id = ObjectId(*raw);
                let token =
                    birth(id).ok_or_else(|| format!("named cache object {raw} is absent"))?;
                Some(NamedScriptTarget {
                    id,
                    birth: Arc::downgrade(&token),
                })
            };
            entries.push(NamedRequest {
                name: name.clone(),
                target,
            });
        }
        Ok(Self { entries })
    }
}

impl super::GameLogic {
    pub(crate) fn initialize_receiving_named_script_cache(&mut self) {
        self.named_script_cache
            .seed(self.objects.registered_values().map(|object| {
                (
                    object.name.clone(),
                    object.id,
                    Arc::clone(&object.visual_draw_identity_token),
                )
            }));
    }
    pub(crate) fn add_receiving_named_script_object(&mut self, id: ObjectId) -> bool {
        let Some(object) = self.objects.get(&id) else {
            return false;
        };
        self.named_script_cache
            .add(&object.name, id, &object.visual_draw_identity_token)
    }
    /// Original transfer never clears the donor's Object.name.
    pub(crate) fn transfer_receiving_script_name(&mut self, name: &str, to: ObjectId) -> bool {
        if name.is_empty() {
            return false;
        }
        let Some(target) = self.objects.get(&to) else {
            return false;
        };
        let named = !target.name.is_empty();
        let birth = Arc::clone(&target.visual_draw_identity_token);
        let old = self.named_script_cache.transfer(name, to, &birth, named);
        let color = old.and_then(|old| {
            self.objects
                .get(&old.id)
                .filter(|object| old.matches(object.id, &object.visual_draw_identity_token))
                .map(|object| object.custom_indicator_color)
        });
        let target = self
            .objects
            .get_mut(&to)
            .expect("validated transfer target");
        target.name = name.into();
        // An absent row leaves the target indicator untouched. A matching NULL
        // row likewise has no donor; a live donor may explicitly clear color.
        if let Some(color) = color {
            target.custom_indicator_color = color;
        }
        true
    }
}

impl PartialEq for NamedScriptCache {
    fn eq(&self, other: &Self) -> bool {
        self.wire() == other.wire()
    }
}
impl Eq for NamedScriptCache {}

#[cfg(test)]
#[path = "named_script_cache_tests.rs"]
mod cache_tests;
