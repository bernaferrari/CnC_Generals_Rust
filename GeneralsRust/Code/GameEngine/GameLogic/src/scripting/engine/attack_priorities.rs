//! Legacy Xfer encoding for simulation attack priorities.
use super::{
    Arc, AttackPriorityInfo, AttackPriorityTable, MAX_ATTACK_PRIORITIES, TheThingFactory, Xfer,
    XferMode, XferSnapshot, XferStatus, XferVersion,
};

/// Keep the original ushort rows, row snapshots, then int count wire order.
pub(super) fn xfer_attack_priorities(
    table: &mut AttackPriorityTable,
    xfer: &mut dyn Xfer,
) -> Result<(), XferStatus> {
    // Host-tail restore is not constrained by C++'s fixed array. Check
    // before narrowing to ushort, including counts that would wrap.
    if matches!(xfer.get_xfer_mode(), XferMode::Save | XferMode::Crc)
        && table.entries().len() > MAX_ATTACK_PRIORITIES
    {
        return Err(XferStatus::InvalidData);
    }
    let mut attack_priority_size: u16 =
        if matches!(xfer.get_xfer_mode(), XferMode::Save | XferMode::Crc) {
            table.entries().len() as u16
        } else {
            0
        };
    xfer.xfer_unsigned_short(&mut attack_priority_size)?;
    if attack_priority_size as usize > MAX_ATTACK_PRIORITIES {
        return Err(XferStatus::InvalidData);
    }
    if xfer.get_xfer_mode() == XferMode::Load {
        // Stage rows so a malformed load leaves the live table untouched.
        let size = attack_priority_size as usize;
        let mut rows = Vec::with_capacity(size);
        rows.resize_with(size, AttackPriorityInfo::new);
        for row in rows.iter_mut() {
            row.xfer(xfer)?;
        }
        let mut num_attack_info: i32 = 0;
        xfer.xfer_int(&mut num_attack_info)?;
        // Unlike C++, reject inconsistent counts before publishing rows.
        if num_attack_info < 1
            || num_attack_info as usize > MAX_ATTACK_PRIORITIES
            || num_attack_info as usize != size
        {
            return Err(XferStatus::InvalidData);
        }
        table.restore(rows);
        return Ok(());
    }
    for row in &mut table.legacy_rows_mut()[..attack_priority_size as usize] {
        Arc::make_mut(row).xfer(xfer)?;
    }

    let mut num_attack_info = table.entries().len() as i32;
    xfer.xfer_int(&mut num_attack_info)?;

    Ok(())
}

impl XferSnapshot for AttackPriorityInfo {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        self.xfer(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;

        xfer.xfer_ascii_string(&mut self.name)?;
        xfer.xfer_int(&mut self.default_priority)?;

        let mut priority_map_count: u16 =
            if xfer.get_xfer_mode() == game_engine::system::XferMode::Save {
                self.priority_map.len().min(u16::MAX as usize) as u16
            } else {
                0
            };
        xfer.xfer_unsigned_short(&mut priority_map_count)?;

        if xfer.get_xfer_mode() == game_engine::system::XferMode::Save {
            let mut count_written: u16 = 0;
            for (template_name, priority) in self.priority_map.iter() {
                count_written = count_written.saturating_add(1);
                let mut name = template_name.clone();
                xfer.xfer_ascii_string(&mut name)?;
                let mut value = *priority;
                xfer.xfer_int(&mut value)?;
            }
            if count_written != priority_map_count {
                return Err(XferStatus::InvalidData);
            }
        } else {
            self.priority_map.clear();
            for _ in 0..priority_map_count {
                let mut name = String::new();
                xfer.xfer_ascii_string(&mut name)?;
                if TheThingFactory::find_template(name.as_str()).is_none() {
                    return Err(XferStatus::InvalidData);
                }
                let mut priority = 0;
                xfer.xfer_int(&mut priority)?;
                self.priority_map.insert(name, priority);
            }
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), XferStatus> {
        Ok(())
    }
}
