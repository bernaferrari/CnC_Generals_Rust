//! Positional BuildListInfo continuation for the current save format.

use super::{AsciiString, BuildListInfo};
use game_engine::system::{Xfer, XferMode};

impl BuildListInfo {
    pub fn crc_xfer(&self, xfer: &mut dyn Xfer) {
        let mut version: u8 = 2;
        let _ = xfer.xfer_version(&mut version, 2);
        let mut v = self.building_name.as_str().to_string();
        let _ = xfer.xfer_ascii_string(&mut v);
        let mut v = self.template_name.as_str().to_string();
        let _ = xfer.xfer_ascii_string(&mut v);
        let mut v = self.location;
        let _ = xfer.xfer_real(&mut v.x);
        let _ = xfer.xfer_real(&mut v.y);
        let _ = xfer.xfer_real(&mut v.z);
        let mut v = self.rally_point_offset;
        let _ = xfer.xfer_real(&mut v.x);
        let _ = xfer.xfer_real(&mut v.y);
        let mut v = self.angle;
        let _ = xfer.xfer_real(&mut v);
        let mut v = self.initially_built;
        let _ = xfer.xfer_bool(&mut v);
        let mut v = self.num_rebuilds;
        let _ = xfer.xfer_unsigned_int(&mut v);
        let mut v = self.script.as_str().to_string();
        let _ = xfer.xfer_ascii_string(&mut v);
        let mut v = self.health;
        let _ = xfer.xfer_int(&mut v);
        let mut v = self.whiner;
        let _ = xfer.xfer_bool(&mut v);
        let mut v = self.unsellable;
        let _ = xfer.xfer_bool(&mut v);
        let mut v = self.repairable;
        let _ = xfer.xfer_bool(&mut v);
        let mut v = self.automatically_build;
        let _ = xfer.xfer_bool(&mut v);
        let mut v = self.object_id;
        let _ = xfer.xfer_object_id(&mut v);
        let mut v = self.object_timestamp;
        let _ = xfer.xfer_unsigned_int(&mut v);
        let mut v = self.under_construction;
        let _ = xfer.xfer_bool(&mut v);
    }

    pub fn transfer_xfer(&mut self, xfer: &mut dyn Xfer) {
        let mut version: u8 = 2;
        let _ = xfer.xfer_version(&mut version, 2);

        let mut building_name = self.building_name.as_str().to_string();
        let _ = xfer.xfer_ascii_string(&mut building_name);
        if xfer.get_xfer_mode() == XferMode::Load {
            self.building_name = AsciiString::from(&building_name);
        }

        let mut template_name = self.template_name.as_str().to_string();
        let _ = xfer.xfer_ascii_string(&mut template_name);
        if xfer.get_xfer_mode() == XferMode::Load {
            self.template_name = AsciiString::from(&template_name);
        }

        let mut location = self.location;
        let _ = xfer.xfer_real(&mut location.x);
        let _ = xfer.xfer_real(&mut location.y);
        let _ = xfer.xfer_real(&mut location.z);
        self.location = location;

        let mut rally_point_offset = self.rally_point_offset;
        let _ = xfer.xfer_real(&mut rally_point_offset.x);
        let _ = xfer.xfer_real(&mut rally_point_offset.y);
        self.rally_point_offset = rally_point_offset;

        let _ = xfer.xfer_real(&mut self.angle);
        let _ = xfer.xfer_bool(&mut self.initially_built);
        let _ = xfer.xfer_unsigned_int(&mut self.num_rebuilds);

        let mut script = self.script.as_str().to_string();
        let _ = xfer.xfer_ascii_string(&mut script);
        if xfer.get_xfer_mode() == XferMode::Load {
            self.script = AsciiString::from(&script);
        }

        let _ = xfer.xfer_int(&mut self.health);
        let _ = xfer.xfer_bool(&mut self.whiner);
        let _ = xfer.xfer_bool(&mut self.unsellable);
        let _ = xfer.xfer_bool(&mut self.repairable);
        let _ = xfer.xfer_bool(&mut self.automatically_build);

        let _ = xfer.xfer_object_id(&mut self.object_id);
        let _ = xfer.xfer_unsigned_int(&mut self.object_timestamp);
        let _ = xfer.xfer_bool(&mut self.under_construction);

        for entry in &mut self.resource_gatherers {
            let _ = xfer.xfer_object_id(entry);
        }

        let _ = xfer.xfer_bool(&mut self.is_supply_building);
        let _ = xfer.xfer_int(&mut self.desired_gatherers);
        let _ = xfer.xfer_bool(&mut self.priority_build);

        if version >= 2 {
            let _ = xfer.xfer_int(&mut self.current_gatherers);
        } else if xfer.get_xfer_mode() == XferMode::Load {
            self.current_gatherers = 0;
        }
    }
}
