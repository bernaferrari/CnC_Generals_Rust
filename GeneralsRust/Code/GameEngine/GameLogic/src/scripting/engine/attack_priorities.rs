//! ScriptEngine.cpp:6483-6639,8923-8941: ordered, live attack-priority rows.
//! The ScriptEngine owns this table. Callers borrow it through the existing
//! engine operation; this module adds no synchronization or ambient selection.
use super::{AttackPriorityInfo, MAX_ATTACK_PRIORITIES, Xfer, XferMode, XferSnapshot, XferStatus};

pub(super) struct AttackPriorityTable {
    rows: Vec<AttackPriorityInfo>,
    count: usize,
}

impl AttackPriorityTable {
    pub(super) fn new() -> Self {
        Self {
            rows: Vec::with_capacity(MAX_ATTACK_PRIORITIES),
            count: 1,
        }
    }

    pub(super) fn ensure_default(&mut self) {
        if self.rows.is_empty() {
            self.rows.push(AttackPriorityInfo::new());
        }
    }

    pub(super) fn clear(&mut self) {
        self.rows.clear();
        self.count = 1;
    }

    /// C++ searches named rows from1; an empty named row is not the default.
    pub(super) fn get(&self, name: &str) -> Option<&AttackPriorityInfo> {
        if self.rows.is_empty() {
            return None;
        }
        for i in 1..self.count {
            if let Some(info) = self.rows.get(i) {
                if info.name == name {
                    return Some(info);
                }
            }
        }
        self.rows.first()
    }

    /// Keep the row live and release its borrow before template lookup or AI dispatch.
    pub(super) fn with_mut<R>(
        &mut self,
        name: &str,
        add_if_missing: bool,
        f: impl FnOnce(&mut AttackPriorityInfo) -> R,
    ) -> Option<R> {
        if self.rows.is_empty() {
            self.rows.push(AttackPriorityInfo::new());
        }
        if self.count == 0 {
            self.count = 1;
        }

        let existing_index = (1..self.count).find(|&i| {
            self.rows
                .get(i)
                .map(|info| info.name == name)
                .unwrap_or(false)
        });
        if let Some(index) = existing_index {
            return self.rows.get_mut(index).map(f);
        }

        if add_if_missing && self.count < MAX_ATTACK_PRIORITIES {
            let mut info = AttackPriorityInfo::new();
            info.name = name.to_string();
            let index = self.count;
            if self.rows.len() <= index {
                self.rows.push(info);
            } else {
                self.rows[index] = info;
            }
            self.count += 1;
            return self.rows.get_mut(index).map(f);
        }

        None
    }

    pub(super) fn entries(&self) -> &[AttackPriorityInfo] {
        &self.rows[..self.count.min(self.rows.len())]
    }

    pub(super) fn restore(&mut self, rows: Vec<AttackPriorityInfo>) {
        self.rows = rows;
        self.ensure_default();
        self.count = self.rows.len();
    }

    /// Keep the original ushort rows, row snapshots, then int count wire order.
    pub(super) fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        let mut attack_priority_size: u16 =
            if matches!(xfer.get_xfer_mode(), XferMode::Save | XferMode::Crc) {
                self.count as u16
            } else {
                0
            };
        xfer.xfer_unsigned_short(&mut attack_priority_size)?;
        if attack_priority_size as usize > MAX_ATTACK_PRIORITIES {
            return Err(XferStatus::InvalidParameters);
        }
        if xfer.get_xfer_mode() == XferMode::Load {
            self.rows.clear();
            self.rows
                .resize_with(attack_priority_size as usize, AttackPriorityInfo::new);
        }
        for i in 0..attack_priority_size as usize {
            self.rows[i].xfer(xfer)?;
        }

        let mut num_attack_info = self.count as i32;
        xfer.xfer_int(&mut num_attack_info)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            self.count = num_attack_info as usize;
        }

        Ok(())
    }
}
