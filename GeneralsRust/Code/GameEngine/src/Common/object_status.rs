// object_status.rs - Object status bitmask type
// Faithful port from ObjectScriptStatusBits.h

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ObjectStatusMaskType {
    mask: u32,
}

impl ObjectStatusMaskType {
    pub fn new() -> Self {
        Self { mask: 0 }
    }

    pub fn with_mask(mask: u32) -> Self {
        Self { mask }
    }

    pub fn set(&mut self, other: ObjectStatusMaskType) {
        self.mask = other.mask;
    }

    pub fn set_bit(&mut self, bit: u32) {
        self.mask |= bit;
    }

    pub fn clear_bit(&mut self, bit: u32) {
        self.mask &= !bit;
    }

    pub fn has_bit(&self, bit: u32) -> bool {
        (self.mask & bit) != 0
    }

    pub fn get_mask(&self) -> u32 {
        self.mask
    }
}

// Object status bits from ObjectScriptStatusBits.h
pub const OBJECT_STATUS_SCRIPT_DISABLED: u32 = 0x01;
pub const OBJECT_STATUS_SCRIPT_UNPOWERED: u32 = 0x02;
pub const OBJECT_STATUS_SCRIPT_UNSELLABLE: u32 = 0x04;
pub const OBJECT_STATUS_SCRIPT_UNSTEALTHED: u32 = 0x08;
pub const OBJECT_STATUS_SCRIPT_TARGETABLE: u32 = 0x10;
