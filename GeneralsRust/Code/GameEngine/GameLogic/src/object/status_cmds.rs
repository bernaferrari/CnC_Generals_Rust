//! Object status/geometry/selectability helpers (C++ Object.cpp).
//!
//! Lives beside `object_status.rs` so the live status file stays a thin
//! dispatcher and Fix02 can keep growing `object.rs` with only `mod` lines.

#![allow(unused_imports)]

use super::object_impl_imports::*;
use super::*;

impl Object {
    /// C++ `Object::setGeometryInfo` (Object.cpp:746-760).
    pub fn set_geometry_info(&mut self, geom: GeometryInfo) {
        self.geometry_info = geom;
        if self.partition_data.is_some() {
            if let Some(partition) = crate::helpers::ThePartitionManager::get() {
                partition.unregister_object(self.id);
                partition.register_object_at(self.id, *self.get_position());
            }
        }
        if let Some(drawable) = &self.drawable {
            if let Ok(mut draw_guard) = drawable.write() {
                draw_guard.react_to_geometry_change();
            }
        }
    }

    /// C++ `Object::setGeometryInfoZ` (Object.cpp:764-770).
    pub fn set_geometry_info_z(&mut self, new_z: Real) {
        self.geometry_info.set_max_height_above_position(new_z);
        if let Some(drawable) = &self.drawable {
            if let Ok(mut draw_guard) = drawable.write() {
                draw_guard.react_to_geometry_change();
            }
        }
    }

    /// C++ `Object::setSelectable` (Object.cpp:2991-2997).
    pub fn set_selectable(&mut self, selectable: bool) {
        self.is_selectable = selectable;
        if let Some(drawable) = &self.drawable {
            if let Ok(mut draw_guard) = drawable.write() {
                draw_guard.set_selectable(selectable);
            }
        }
    }

    /// C++ `Object::hasSingleUseCommandBeenUsed`.
    pub fn has_single_use_command_been_used(&self) -> bool {
        self.single_use_command_used
    }

    /// C++ `Object::markSingleUseCommandUsed`.
    pub fn mark_single_use_command_used(&mut self) {
        self.single_use_command_used = true;
    }

    /// C++ `Object::isScriptUnsellable` / `testScriptStatusBit(OBJECT_STATUS_SCRIPT_UNSELLABLE)`.
    pub fn is_script_unsellable(&self) -> bool {
        self.test_script_status_bit(ObjectScriptStatusBit::Unsellable)
    }

    /// C++ `Object::setStatus` REPULSOR arm (Object.cpp:965-970).
    pub(super) fn wake_repulsor_helper_for_status(&mut self) {
        let Some(helper) = &self.repulsor_helper else {
            return;
        };
        let wake_frame =
            crate::helpers::TheGameLogic::get_frame().saturating_add(2 * LOGICFRAMES_PER_SECOND);
        if let Ok(mut guard) = helper.lock() {
            guard.wake_for_clear(wake_frame);
        }
    }
}
