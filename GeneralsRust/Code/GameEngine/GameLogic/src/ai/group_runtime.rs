//! AiGroup owned fields, inert construction and test-only observation.
//! Behavior remains in the parent module; public identity stays crate::ai::AiGroup.
use super::{ObjectId, PathId, Real};

#[allow(dead_code)]
#[derive(Debug)]
pub struct AiGroup {
    #[cfg(test)]
    pub(super) speed_recompute_visits: Vec<ObjectId>,
    #[cfg(test)]
    pub(super) speed_borrowed_hits: usize,
    pub(super) id: u32,
    pub(super) member_list: Vec<ObjectId>,
    pub(super) speed: Real,
    pub(super) dirty: bool,
    pub(super) ground_path: Option<PathId>,
}

impl AiGroup {
    #[cfg(test)]
    pub(crate) fn speed_dirty_for_test(&self) -> bool {
        self.dirty
    }
    #[cfg(test)]
    pub(crate) fn speed_visits_for_test(&self) -> &[ObjectId] {
        &self.speed_recompute_visits
    }

    #[cfg(test)]
    pub(crate) fn speed_borrowed_hits_for_test(&self) -> usize {
        self.speed_borrowed_hits
    }

    pub fn new(id: u32) -> Self {
        Self {
            #[cfg(test)]
            speed_recompute_visits: Vec::new(),
            #[cfg(test)]
            speed_borrowed_hits: 0,
            id,
            member_list: Vec::new(),
            speed: 0.0,
            dirty: true,
            ground_path: None,
        }
    }

    /// Observation is per-instance and test-only. Production has no state
    /// or side effects here; field order and the runtime representation stay put.
    #[inline]
    pub(super) fn observe_speed_member(&mut self, _member: ObjectId) {
        #[cfg(test)]
        {
            self.speed_recompute_visits.push(_member);
            eprintln!(
                "FORMATION_RECOMPUTE group={} member={} dirty={}",
                self.id, _member, self.dirty
            );
        }
    }

    #[inline]
    pub(super) fn observe_borrowed_speed_hit(&mut self) {
        #[cfg(test)]
        {
            self.speed_borrowed_hits += 1;
        }
    }
}
