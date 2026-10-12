//! C++ AIGroup speed and center observations from a supplied session's members.
use super::{AiGroup, ObjectId, Real};
use crate::common::Coord3D;

pub(crate) struct GroupMotionMember {
    pub position: Coord3D,
    pub held: bool,
    pub has_ai: bool,
    pub speed: Option<Real>,
}

impl AiGroup {
    /// The callback resolves one canonical member. `needs_speed` is false when
    /// the original cached group speed is clean; callers need not borrow its AI.
    pub(crate) fn motion_with_members(
        &mut self,
        mut member: impl FnMut(ObjectId, bool) -> Result<Option<GroupMotionMember>, String>,
    ) -> Result<(Real, Option<Coord3D>), String> {
        let needs_speed = self.dirty;
        let mut minimum = Real::INFINITY;
        let mut ai_sum = Coord3D::new(0.0, 0.0, 0.0);
        let mut all_sum = Coord3D::new(0.0, 0.0, 0.0);
        let mut ai_count = 0u32;
        let mut all_count = 0u32;
        for &id in &self.member_list {
            let Some(value) = member(id, needs_speed)? else {
                continue;
            };
            if needs_speed {
                if let Some(speed) = value.speed {
                    minimum = minimum.min(speed.max(0.0));
                }
            }
            if value.held {
                continue;
            }
            all_sum.x += value.position.x;
            all_sum.y += value.position.y;
            all_sum.z += value.position.z;
            all_count += 1;
            if value.has_ai {
                ai_sum.x += value.position.x;
                ai_sum.y += value.position.y;
                ai_sum.z += value.position.z;
                ai_count += 1;
            }
        }
        // Publish a recomputed cache only after every admitted lookup succeeded.
        if needs_speed {
            self.speed = if minimum.is_finite() { minimum } else { 0.0 };
            self.dirty = false;
        }
        let (mut center, count) = if ai_count > 0 {
            (ai_sum, ai_count)
        } else {
            (all_sum, all_count)
        };
        let center = if count == 0 {
            None
        } else {
            center.x /= count as Real;
            center.y /= count as Real;
            center.z /= count as Real;
            Some(center)
        };
        Ok((self.speed, center))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn owned_motion_preserves_member_order_speed_and_ai_center_preference() {
        let mut group = AiGroup::new(11);
        group.member_list = vec![3, 1, 2];
        let mut visits = Vec::new();
        let (speed, center) = group
            .motion_with_members(|id, needs_speed| {
                visits.push(id);
                assert!(needs_speed);
                Ok(Some(GroupMotionMember {
                    position: Coord3D::new(id as Real * 10.0, 0.0, 0.0),
                    held: id == 3,
                    has_ai: id != 2,
                    speed: (id != 2).then_some(id as Real),
                }))
            })
            .unwrap();
        assert_eq!(visits, vec![3, 1, 2]);
        assert_eq!(speed, 1.0);
        assert_eq!(center, Some(Coord3D::new(10.0, 0.0, 0.0)));
        group
            .motion_with_members(|_, needs_speed| {
                assert!(!needs_speed);
                Ok(None)
            })
            .unwrap();
        assert_eq!(group.speed, 1.0);
    }
    #[test]
    fn held_members_keep_group_speed_but_have_no_center() {
        let mut group = AiGroup::new(13);
        group.add(1);
        let (speed, center) = group
            .motion_with_members(|_, _| {
                Ok(Some(GroupMotionMember {
                    position: Coord3D::new(100.0, 200.0, 0.0),
                    held: true,
                    has_ai: true,
                    speed: Some(2.0),
                }))
            })
            .unwrap();
        assert_eq!(speed, 2.0);
        assert_eq!(center, None);
    }
    #[test]
    fn failed_owned_member_observation_does_not_publish_speed_cache() {
        let mut group = AiGroup::new(12);
        group.member_list = vec![1];
        assert!(
            group
                .motion_with_members(|_, _| Err("wrong session".into()))
                .is_err()
        );
        assert!(group.dirty);
        assert_eq!(group.speed, 0.0);
    }
}
