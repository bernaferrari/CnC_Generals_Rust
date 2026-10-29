//! Session-owned AIGroup membership for the migrated team-hunt route.
//!
//! Group speed and other group-producing commands are separate migration work.
//! Object.cpp omits its group pointer from saves; groups are created as needed.
//! Group membership is not inferred from a formation ID.
use std::collections::HashMap;
use std::sync::{Arc, Weak};

use crate::game_logic::{GameLogic, KindOf, ObjectId};

#[derive(Default)]
pub(crate) struct AiGroupOwner {
    next_id: u32,
    groups: HashMap<u32, Vec<ObjectId>>,
    membership: HashMap<ObjectId, Membership>,
}

struct Membership {
    group: u32,
    birth: Weak<()>,
}

impl AiGroupOwner {
    fn create(&mut self) -> u32 {
        let start = self.next_id;
        loop {
            self.next_id = self.next_id.wrapping_add(1);
            let id = self.next_id;
            if let std::collections::hash_map::Entry::Vacant(slot) = self.groups.entry(id) {
                slot.insert(Vec::new());
                return id;
            }
            assert_ne!(id, start, "AI group ID space exhausted");
        }
    }

    fn add(&mut self, group: u32, object: ObjectId, birth: Weak<()>) {
        // AIGroup::add appends before Object::enterGroup leaves its previous
        // group. In particular, re-adding to the same group moves the member
        // to the end without destroying the group when it was its only member.
        self.groups
            .get_mut(&group)
            .expect("new AI group")
            .push(object);
        if let Some(previous) = self.membership.remove(&object) {
            let previous = previous.group;
            let previous_members = self.groups.get_mut(&previous).expect("live AI group");
            let index = previous_members
                .iter()
                .position(|&member| member == object)
                .expect("AI group membership is reciprocal");
            previous_members.remove(index);
            if previous_members.is_empty() {
                self.groups.remove(&previous);
            }
        }
        self.membership.insert(object, Membership { group, birth });
    }

    fn remove(&mut self, object: ObjectId) -> bool {
        let Some(membership) = self.membership.remove(&object) else {
            return false;
        };
        let group = membership.group;
        let members = self.groups.get_mut(&group).expect("live AI group");
        let index = members
            .iter()
            .position(|&member| member == object)
            .expect("AI group membership is reciprocal");
        members.remove(index);
        if members.is_empty() {
            self.groups.remove(&group);
        }
        true
    }

    fn members(&self, group: u32) -> Option<&[ObjectId]> {
        self.groups.get(&group).map(Vec::as_slice)
    }

    pub(crate) fn clear_groups(&mut self) {
        self.groups.clear();
        self.membership.clear();
    }

    pub(crate) fn reset(&mut self) {
        self.clear_groups();
        self.next_id = 0;
    }
}

impl GameLogic {
    /// Team::getTeamAsAIGroup in roster order. A non-AI object joins only if
    /// it is a structure or always selectable, just as AIGroup::add requires.
    pub(crate) fn create_team_ai_group(&mut self, members: &[u32]) -> u32 {
        let group = self.ai_groups.create();
        for &member in members {
            let id = ObjectId(member);
            let birth = self.objects.get(&id).and_then(|object| {
                (object.has_ai_update_interface()
                    || object.is_kind_of(KindOf::Structure)
                    || object.is_kind_of(KindOf::AlwaysSelectable))
                .then(|| Arc::downgrade(&object.visual_draw_identity_token))
            });
            if let Some(birth) = birth {
                self.ai_groups.add(group, id, birth);
            }
        }
        group
    }

    pub(crate) fn leave_ai_group(&mut self, object: ObjectId) -> bool {
        let Some(birth) = self
            .objects
            .get(&object)
            .map(|object| Arc::downgrade(&object.visual_draw_identity_token))
        else {
            return false;
        };
        self.leave_ai_group_with_birth(object, &birth)
    }

    pub(crate) fn leave_ai_group_with_birth(&mut self, object: ObjectId, birth: &Weak<()>) -> bool {
        if self
            .ai_groups
            .membership
            .get(&object)
            .is_some_and(|membership| membership.birth.ptr_eq(birth))
        {
            self.ai_groups.remove(object)
        } else {
            false
        }
    }

    /// The group currently owning this object's birth, if any. IDs are scoped
    /// to this GameLogic and may be equal in a concurrent session.
    pub fn ai_group_id_of(&self, object: ObjectId) -> Option<u32> {
        let membership = self.ai_groups.membership.get(&object)?;
        let current = self.objects.get(&object)?;
        membership
            .birth
            .ptr_eq(&Arc::downgrade(&current.visual_draw_identity_token))
            .then_some(membership.group)
    }

    /// Ordered live membership of this object's group.
    pub fn ai_group_members_of(&self, object: ObjectId) -> Option<Vec<ObjectId>> {
        Some(self.ai_group_members(self.ai_group_id_of(object)?))
    }

    pub(crate) fn ai_group_members(&self, group: u32) -> Vec<ObjectId> {
        self.ai_groups
            .members(group)
            .unwrap_or_default()
            .iter()
            .copied()
            .filter(|&object| self.ai_group_id_of(object) == Some(group))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_id_wrap_preserves_live_groups() {
        let mut groups = AiGroupOwner::default();
        let birth = Arc::new(());
        let first = groups.create();
        groups.add(first, ObjectId(1), Arc::downgrade(&birth));
        groups.next_id = u32::MAX;
        let zero = groups.create();
        let after_wrap = groups.create();
        assert_eq!(zero, 0);
        assert_eq!(after_wrap, 2);
        assert_eq!(groups.members(first), Some(&[ObjectId(1)][..]));
    }

    #[test]
    fn reentry_appends_before_unlink_and_last_leave_retires_group() {
        let mut groups = AiGroupOwner::default();
        let birth = Arc::new(());
        let first = groups.create();
        groups.add(first, ObjectId(1), Arc::downgrade(&birth));
        groups.add(first, ObjectId(2), Arc::downgrade(&birth));
        groups.add(first, ObjectId(1), Arc::downgrade(&birth));
        assert_eq!(groups.members(first), Some(&[ObjectId(2), ObjectId(1)][..]));
        let second = groups.create();
        groups.add(second, ObjectId(2), Arc::downgrade(&birth));
        groups.add(second, ObjectId(1), Arc::downgrade(&birth));
        assert_eq!(groups.members(first), None);
        assert_eq!(
            groups.members(second),
            Some(&[ObjectId(2), ObjectId(1)][..])
        );
        assert!(groups.remove(ObjectId(2)));
        groups.add(second, ObjectId(1), Arc::downgrade(&birth));
        assert_eq!(groups.members(second), Some(&[ObjectId(1)][..]));
        assert!(groups.remove(ObjectId(1)));
        assert_eq!(groups.members(second), None);
    }
}
