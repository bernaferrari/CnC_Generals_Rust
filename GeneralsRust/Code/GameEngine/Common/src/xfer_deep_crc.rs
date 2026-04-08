// FILE: xfer_deep_crc.rs
// Ported from: GeneralsMD/Code/GameEngine/Include/Common/XferDeepCRC.h + Source/Common/System/XferDeepCRC.cpp
// Author: Colin Day, February 2002
//
// PARITY_NOTE: Full XferDeepCRC implementation exists in common::system::xfer_crc
// with CRC tracking, object hierarchy, and corruption detection.

pub use crate::common::system::xfer_crc::{CorruptionEntry, XferCRC, XferDeepCRC};
