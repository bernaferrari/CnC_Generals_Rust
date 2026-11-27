//! Ordered script names and live object receipts (C++ ScriptEngine).
use generals_game_domain::ObjectId;
use std::collections::HashMap;
use std::sync::{Arc, Weak};

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum NamedCacheError {
    #[error("named cache exceeds original u16 count")]
    TooManyEntries,
    #[error("invalid named cache name")]
    InvalidName,
    #[error("named cache object {0} is absent")]
    MissingObject(ObjectId),
    #[error("stale named cache object {id} for {name:?}")]
    StaleObject { name: String, id: ObjectId },
}

fn validate_count(count: usize) -> Result<(), NamedCacheError> {
    if count > u16::MAX as usize {
        return Err(NamedCacheError::TooManyEntries);
    }
    Ok(())
}

fn validate_name(name: &str) -> Result<(), NamedCacheError> {
    if name.contains('\0') {
        return Err(NamedCacheError::InvalidName);
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub struct NamedScriptTarget {
    pub id: ObjectId,
    birth: Weak<()>,
}
impl NamedScriptTarget {
    fn new(id: ObjectId, birth: &Arc<()>) -> Self {
        Self {
            id,
            birth: Arc::downgrade(birth),
        }
    }

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
pub struct NamedScriptCache {
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
                target: Some(NamedScriptTarget::new(id, &birth)),
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
                    row.target = Some(NamedScriptTarget::new(id, birth));
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
            target: Some(NamedScriptTarget::new(id, birth)),
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
        row.target.replace(NamedScriptTarget::new(id, birth))
    }
    pub fn lookup(&self, name: &str) -> Option<&NamedScriptTarget> {
        self.entries
            .iter()
            .find(|row| row.name == name)
            .and_then(|row| row.target.as_ref())
    }
    /// C++ didUnitExist checks whether the first matching row is null.
    pub fn did_exist(&self, name: &str) -> bool {
        self.entries
            .iter()
            .find(|row| row.name == name)
            .is_some_and(|row| row.target.is_none())
    }
    pub fn clear(&mut self) {
        self.entries.clear();
    }
    pub fn contains(&self, name: &str) -> bool {
        self.entries.iter().any(|row| row.name == name)
    }

    /// A first tombstone or stale target hides any later duplicate.
    pub fn project_live_ids(
        &self,
        mut is_live: impl FnMut(&NamedScriptTarget) -> bool,
    ) -> HashMap<String, u32> {
        let mut names = HashMap::with_capacity(self.entries.len());
        for row in &self.entries {
            names.entry(row.name.clone()).or_insert_with(|| {
                row.target
                    .as_ref()
                    .filter(|target| is_live(target))
                    .map_or(0, |target| target.id.0)
            });
        }
        names.retain(|_, id| *id != 0);
        names
    }

    /// An expired receipt cannot be serialized as an ID: that ID can now be
    /// absent or refer to a different birth on restore.
    pub fn validate_for_save(
        &self,
        mut is_live: impl FnMut(&NamedScriptTarget) -> bool,
    ) -> Result<(), NamedCacheError> {
        validate_count(self.entries.len())?;
        for row in &self.entries {
            validate_name(&row.name)?;
            if let Some(target) = &row.target
                && (target.id.0 == 0 || !is_live(target))
            {
                return Err(NamedCacheError::StaleObject {
                    name: row.name.clone(),
                    id: target.id,
                });
            }
        }
        Ok(())
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
    ) -> Result<Self, NamedCacheError> {
        validate_count(rows.len())?;
        let mut entries = Vec::with_capacity(rows.len());
        for (name, raw) in rows {
            validate_name(name)?;
            let target = if *raw == 0 {
                None
            } else {
                let id = ObjectId(*raw);
                let token = birth(id).ok_or(NamedCacheError::MissingObject(id))?;
                Some(NamedScriptTarget::new(id, &token))
            };
            entries.push(NamedRequest {
                name: name.clone(),
                target,
            });
        }
        Ok(Self { entries })
    }
}

impl PartialEq for NamedScriptCache {
    fn eq(&self, other: &Self) -> bool {
        self.wire() == other.wire()
    }
}
impl Eq for NamedScriptCache {}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_tail_empty_name_row_and_first_null_are_retained() {
        let birth = Arc::new(());
        let rows = vec![(String::new(), 0), (String::new(), 7)];
        let cache =
            NamedScriptCache::from_wire(&rows, |id| (id == ObjectId(7)).then(|| birth.clone()))
                .unwrap();
        assert_eq!(cache.wire(), rows);
        assert!(cache.did_exist(""));
        assert!(cache.lookup("").is_none());
    }
    #[test]
    fn first_null_duplicate_reuse_and_pointer_before_later_name_match_follow_source_visits() {
        let a = Arc::new(());
        let b = Arc::new(());
        let c = Arc::new(());
        let mut cache = NamedScriptCache::default();
        cache.seed([
            ("same".into(), ObjectId(1), a.clone()),
            ("same".into(), ObjectId(2), b.clone()),
        ]);
        cache.remove(ObjectId(1), &a);
        assert!(cache.did_exist("same"));
        assert!(cache.lookup("same").is_none());
        assert!(cache.add("same", ObjectId(3), &c));
        assert!(cache.lookup("same").unwrap().matches(ObjectId(3), &c));
        assert!(!cache.add("same", ObjectId(2), &b));
        cache.seed([
            ("old".into(), ObjectId(1), a.clone()),
            ("new".into(), ObjectId(2), b.clone()),
        ]);
        assert!(cache.add("new", ObjectId(1), &a));
        assert_eq!(cache.wire(), [("new".into(), 1), ("new".into(), 2)]);
    }
    #[test]
    fn transfer_preserves_target_old_tombstone_and_existing_birth_not_reused_id() {
        let a = Arc::new(());
        let b = Arc::new(());
        let replacement = Arc::new(());
        let mut cache = NamedScriptCache::default();
        cache.seed([
            ("donor".into(), ObjectId(1), a.clone()),
            ("target".into(), ObjectId(2), b.clone()),
        ]);
        assert!(
            cache
                .transfer("donor", ObjectId(2), &b, true)
                .unwrap()
                .matches(ObjectId(1), &a)
        );
        assert_eq!(cache.wire(), [("donor".into(), 2), ("target".into(), 0)]);
        cache.remove(ObjectId(2), &replacement);
        assert!(cache.lookup("donor").unwrap().matches(ObjectId(2), &b));
        cache.remove(ObjectId(2), &b);
        assert!(cache.did_exist("donor"));
    }
}
