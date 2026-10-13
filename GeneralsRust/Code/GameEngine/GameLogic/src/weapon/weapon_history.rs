//! Weapon.cpp:1169–1251 mutable hit history belongs to the receiving match.
//! Definitions and their clones share immutable identity; their hits never do.
use super::{HistoricWeaponDamageInfo, WeaponTemplate};
use crate::common::Coord3D;
use std::collections::VecDeque;
use std::sync::Arc;

#[derive(Debug)]
pub(super) struct WeaponHistory {
    identity: Arc<()>,
    hits: VecDeque<HistoricWeaponDamageInfo>,
}
#[derive(Default, Debug)]
pub(super) struct WeaponHistories {
    rows: Vec<WeaponHistory>,
}
impl WeaponHistories {
    fn row_mut(&mut self, template: &WeaponTemplate) -> &mut WeaponHistory {
        if let Some(index) = self
            .rows
            .iter()
            .position(|row| Arc::ptr_eq(&row.identity, template.history_identity()))
        {
            return &mut self.rows[index];
        }
        self.rows.push(WeaponHistory {
            identity: template.history_identity().clone(),
            hits: VecDeque::new(),
        });
        self.rows.last_mut().unwrap()
    }
    pub(super) fn trim(&mut self, template: &WeaponTemplate, frame: u32, limit: u32) {
        let expiry = frame.wrapping_sub(limit);
        let row = self.row_mut(template);
        while row.hits.front().is_some_and(|hit| hit.frame <= expiry) {
            row.hits.pop_front();
        }
    }
    /// True requests a synchronous bonus fire. Clear only after that callback.
    pub(super) fn observe(
        &mut self,
        template: &WeaponTemplate,
        position: &Coord3D,
        frame: u32,
        limit: u32,
        bonus_is_self: bool,
    ) -> bool {
        self.trim(template, frame, limit);
        if template.historic_bonus_count <= 0 || bonus_is_self {
            return false;
        }
        let oldest = frame.wrapping_sub(template.historic_bonus_time);
        let radius = template.historic_bonus_radius * template.historic_bonus_radius;
        let row = self.row_mut(template);
        let count = row
            .hits
            .iter()
            .filter(|hit| {
                let dx = hit.location.x - position.x;
                let dy = hit.location.y - position.y;
                hit.frame >= oldest && dx * dx + dy * dy <= radius
            })
            .count() as i32;
        if count >= template.historic_bonus_count - 1 {
            true
        } else {
            row.hits
                .push_back(HistoricWeaponDamageInfo::new(frame, *position));
            false
        }
    }
    pub(super) fn clear(&mut self, template: &WeaponTemplate) {
        self.row_mut(template).hits.clear();
    }
    pub(super) fn len(&self, template: &WeaponTemplate) -> usize {
        self.rows
            .iter()
            .find(|row| Arc::ptr_eq(&row.identity, template.history_identity()))
            .map_or(0, |row| row.hits.len())
    }
}
