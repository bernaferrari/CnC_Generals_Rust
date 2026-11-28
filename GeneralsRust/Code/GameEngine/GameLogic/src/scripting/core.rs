//! Script program re-exports and original save continuation.

use crate::scripting::XferSnapshot;
use game_engine::common::system::{Xfer, XferStatus, XferVersion};

pub use super::chunk_codec::{ScriptListReadInfo, parse_player_scripts_list_chunk};
pub use game_engine::common::script_templates::{ActionTemplate, ConditionTemplate, Template};
pub use generals_sim::scripting::program::*;

/// C++ ScriptEngine.cpp:5810 / 5935 — THE_PLAYER and teamThePlayer are
/// Challenge-only aliases. Regular single-player maps author a real side
/// named ThePlayer that must not be remapped to the local general.
pub fn is_generals_challenge_campaign(context: &crate::scripting::HostTriggerWorld) -> bool {
    context.is_generals_challenge_campaign()
}

/// Allocate a default group name through the receiving compatibility session.
pub fn new_script_group() -> ScriptGroup {
    crate::system::engine_stores::active_services().allocate_script_group()
}

impl XferSnapshot for Script {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        self.xfer(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;

        let mut active = self.is_active;
        xfer.xfer_bool(&mut active)?;
        self.is_active = active;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), XferStatus> {
        Ok(())
    }
}

impl XferSnapshot for ScriptGroup {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        self.xfer(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        let current_version: XferVersion = 2;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;

        if version >= 2 {
            let mut active = self.is_group_active;
            xfer.xfer_bool(&mut active)?;
            self.is_group_active = active;
        }

        let mut script_count: u16 = 0;
        let mut cursor = self.first_script.as_deref();
        while let Some(script) = cursor {
            script_count = script_count.saturating_add(1);
            cursor = script.next_script.as_deref();
        }

        let count_verify = script_count;
        xfer.xfer_unsigned_short(&mut script_count)?;
        let mut remaining = script_count;

        let mut current = self.first_script.as_mut();
        while let Some(script) = current {
            if remaining == 0 {
                break;
            }
            script.as_mut().xfer(xfer)?;
            remaining = remaining.saturating_sub(1);
            current = script.next_script.as_mut();
        }

        if count_verify != script_count {
            // Attempt to recover by consuming any extra serialized scripts.
            while remaining > 0 {
                let mut temp = Script::new();
                temp.xfer(xfer)?;
                remaining = remaining.saturating_sub(1);
            }
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), XferStatus> {
        Ok(())
    }
}

impl XferSnapshot for ScriptList {
    fn crc(&mut self, _xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;

        let mut script_count: u16 = 0;
        let mut cursor = self.first_script.as_deref();
        while let Some(script) = cursor {
            script_count = script_count.saturating_add(1);
            cursor = script.next_script.as_deref();
        }
        let count_verify = script_count;
        xfer.xfer_unsigned_short(&mut script_count)?;
        let mut remaining = script_count;

        let mut current = self.first_script.as_mut();
        while let Some(script) = current {
            if remaining == 0 {
                break;
            }
            script.as_mut().xfer(xfer)?;
            remaining = remaining.saturating_sub(1);
            current = script.next_script.as_mut();
        }

        if count_verify != script_count {
            while remaining > 0 {
                let mut temp = Script::new();
                temp.xfer(xfer)?;
                remaining = remaining.saturating_sub(1);
            }
        }

        let mut group_count: u16 = 0;
        let mut gcur = self.first_group.as_deref();
        while let Some(group) = gcur {
            group_count = group_count.saturating_add(1);
            gcur = group.next_group.as_deref();
        }
        let group_verify = group_count;
        xfer.xfer_unsigned_short(&mut group_count)?;
        let mut group_remaining = group_count;

        let mut current_group = self.first_group.as_mut();
        while let Some(group) = current_group {
            if group_remaining == 0 {
                break;
            }
            group.as_mut().xfer(xfer)?;
            group_remaining = group_remaining.saturating_sub(1);
            current_group = group.next_group.as_mut();
        }

        if group_verify != group_count {
            while group_remaining > 0 {
                let mut temp = new_script_group();
                temp.xfer(xfer)?;
                group_remaining = group_remaining.saturating_sub(1);
            }
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), XferStatus> {
        Ok(())
    }
}
