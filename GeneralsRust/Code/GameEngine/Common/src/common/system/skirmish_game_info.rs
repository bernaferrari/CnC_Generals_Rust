//! Original GameInfo.cpp:1488–1588 record. No runtime owner or ambient hooks.
//! The two existing Xfer traits adapt to one field walk; Main uses memory IO.
use super::xfer::Xfer;
use super::xfer_load::XferLoad;
use super::xfer_save::XferSave;
use std::io::{self, Cursor};

/// C++ `GameInfo.h` / `NetworkDefs.h`: `MAX_SLOTS = 8`.
pub const SKIRMISH_MAX_SLOTS: usize = 8;
pub const SKIRMISH_GAME_INFO_VERSION: u8 = 4;

/// One slot of C++ `SkirmishGameInfo::xfer` (GameInfo.cpp:1488-1588).
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SkirmishSlot {
    pub state: i32,
    pub name: String,
    pub is_accepted: bool,
    pub is_muted: bool,
    pub color: i32,
    pub start_pos: i32,
    pub player_template: i32,
    pub team_number: i32,
    pub orig_color: i32,
    pub orig_start_pos: i32,
    pub orig_player_template: i32,
}

/// C++ `SkirmishGameInfo::xfer` payload used as `TheChallengeGameInfo`.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SkirmishGameInfo {
    pub preorder_mask: i32,
    pub crc_interval: i32,
    pub in_game: bool,
    pub in_progress: bool,
    pub surrendered: bool,
    pub game_id: i32,
    pub slots: [SkirmishSlot; SKIRMISH_MAX_SLOTS],
    pub local_ip: u32,
    pub map_name: String,
    pub map_crc: u32,
    pub map_size: u32,
    pub map_mask: i32,
    pub seed: i32,
    pub superweapon_restriction: u16,
    pub starting_cash: u32,
}

impl Default for SkirmishGameInfo {
    fn default() -> Self {
        Self {
            preorder_mask: 0,
            crc_interval: 0,
            in_game: false,
            in_progress: false,
            surrendered: false,
            game_id: 0,
            slots: Default::default(),
            local_ip: 0,
            map_name: String::new(),
            map_crc: 0,
            map_size: 0,
            map_mask: 0,
            seed: 0,
            superweapon_restriction: 0,
            starting_cash: 0,
        }
    }
}

/// Minimal scalar adapter for the existing System and Common Xfer traits.
pub(crate) trait GameInfoFields {
    type Error;
    fn loading(&self) -> bool;
    fn invalid(&self, message: &str) -> Self::Error;
    fn xfer_version(&mut self, value: &mut u8, maximum: u8) -> Result<(), Self::Error>;
    fn xfer_int(&mut self, value: &mut i32) -> Result<(), Self::Error>;
    fn xfer_unsigned_int(&mut self, value: &mut u32) -> Result<(), Self::Error>;
    fn xfer_unsigned_short(&mut self, value: &mut u16) -> Result<(), Self::Error>;
    fn xfer_bool(&mut self, value: &mut bool) -> Result<(), Self::Error>;
    fn xfer_ascii_string(&mut self, value: &mut String) -> Result<(), Self::Error>;
    fn xfer_unicode_string(&mut self, value: &mut String) -> Result<(), Self::Error>;
}
impl SkirmishGameInfo {
    pub(crate) fn transfer<F: GameInfoFields>(&mut self, xfer: &mut F) -> Result<(), F::Error> {
        let mut version = SKIRMISH_GAME_INFO_VERSION;
        xfer.xfer_version(&mut version, SKIRMISH_GAME_INFO_VERSION)?;

        xfer.xfer_int(&mut self.preorder_mask)?;
        xfer.xfer_int(&mut self.crc_interval)?;
        xfer.xfer_bool(&mut self.in_game)?;
        xfer.xfer_bool(&mut self.in_progress)?;
        xfer.xfer_bool(&mut self.surrendered)?;
        xfer.xfer_int(&mut self.game_id)?;

        let mut slot_count = SKIRMISH_MAX_SLOTS as i32;
        xfer.xfer_int(&mut slot_count)?;
        if slot_count != SKIRMISH_MAX_SLOTS as i32 {
            return Err(xfer.invalid("SkirmishGameInfo requires exactly eight slots"));
        }
        for slot in &mut self.slots {
            xfer.xfer_int(&mut slot.state)?;
            if version >= 2 {
                xfer.xfer_unicode_string(&mut slot.name)?;
            }
            xfer.xfer_bool(&mut slot.is_accepted)?;
            xfer.xfer_bool(&mut slot.is_muted)?;
            xfer.xfer_int(&mut slot.color)?;
            xfer.xfer_int(&mut slot.start_pos)?;
            xfer.xfer_int(&mut slot.player_template)?;
            xfer.xfer_int(&mut slot.team_number)?;
            xfer.xfer_int(&mut slot.orig_color)?;
            xfer.xfer_int(&mut slot.orig_start_pos)?;
            xfer.xfer_int(&mut slot.orig_player_template)?;
        }

        xfer.xfer_unsigned_int(&mut self.local_ip)?;
        xfer.xfer_ascii_string(&mut self.map_name)?;
        xfer.xfer_unsigned_int(&mut self.map_crc)?;
        xfer.xfer_unsigned_int(&mut self.map_size)?;
        xfer.xfer_int(&mut self.map_mask)?;
        xfer.xfer_int(&mut self.seed)?;

        if version >= 3 {
            xfer.xfer_unsigned_short(&mut self.superweapon_restriction)?;
            if version == 3 {
                let mut obsolete = false;
                xfer.xfer_bool(&mut obsolete)?;
            }
            let mut money_version: u8 = 1;
            xfer.xfer_version(&mut money_version, 1)?;
            xfer.xfer_unsigned_int(&mut self.starting_cash)?;
        } else if xfer.loading() {
            self.superweapon_restriction = 0;
            self.starting_cash = 0;
        }
        Ok(())
    }
    pub fn write_to_common(&self, xfer: &mut dyn Xfer) -> io::Result<()> {
        self.clone().transfer(&mut CommonFields(xfer))
    }
    pub fn encode(&self) -> io::Result<Vec<u8>> {
        let mut output = Cursor::new(Vec::new());
        self.clone()
            .transfer(&mut CommonFields(&mut XferSave::new(&mut output, 1)))?;
        Ok(output.into_inner())
    }
    pub fn decode(bytes: &[u8]) -> io::Result<Self> {
        if bytes.first() != Some(&SKIRMISH_GAME_INFO_VERSION) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "expected SkirmishGameInfo version 4",
            ));
        }
        let mut cursor = Cursor::new(bytes);
        let mut value = Self::default();
        value.transfer(&mut CommonFields(&mut XferLoad::new(&mut cursor, 1)))?;
        if cursor.position() != bytes.len() as u64 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "trailing SkirmishGameInfo bytes",
            ));
        }
        Ok(value)
    }
}
struct CommonFields<'a>(&'a mut dyn Xfer);
impl GameInfoFields for CommonFields<'_> {
    type Error = io::Error;
    fn loading(&self) -> bool {
        self.0.get_xfer_mode() == super::xfer::XferMode::Load
    }
    fn invalid(&self, message: &str) -> io::Error {
        io::Error::new(io::ErrorKind::InvalidData, message)
    }
    fn xfer_version(&mut self, value: &mut u8, maximum: u8) -> io::Result<()> {
        self.0.xfer_version(value, maximum)
    }
    fn xfer_int(&mut self, value: &mut i32) -> io::Result<()> {
        self.0.xfer_int(value)
    }
    fn xfer_unsigned_int(&mut self, value: &mut u32) -> io::Result<()> {
        self.0.xfer_unsigned_int(value)
    }
    fn xfer_unsigned_short(&mut self, value: &mut u16) -> io::Result<()> {
        self.0.xfer_unsigned_short(value)
    }
    fn xfer_bool(&mut self, value: &mut bool) -> io::Result<()> {
        self.0.xfer_bool(value)
    }
    fn xfer_ascii_string(&mut self, value: &mut String) -> io::Result<()> {
        self.0.xfer_ascii_string(value)
    }
    fn xfer_unicode_string(&mut self, value: &mut String) -> io::Result<()> {
        self.0.xfer_unicode_string(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_v4_literal_field_order_matches_both_existing_xfer_adapters() {
        let mut info = SkirmishGameInfo::default();
        info.preorder_mask = 0x12345678;
        info.crc_interval = 77;
        info.in_game = true;
        info.surrendered = true;
        info.game_id = -17;
        info.slots[0] = SkirmishSlot {
            state: 5,
            name: "AΩ".into(),
            is_accepted: true,
            is_muted: false,
            color: 3,
            start_pos: 2,
            player_template: 6,
            team_number: -1,
            orig_color: 4,
            orig_start_pos: -1,
            orig_player_template: -1,
        };
        info.local_ip = 0x01020304;
        info.map_name = "Map".into();
        info.map_crc = 91;
        info.map_size = 123;
        info.map_mask = -2;
        info.seed = -3;
        info.superweapon_restriction = 1;
        info.starting_cash = 0xf1234567;
        // Independently spell GameInfo.cpp1490–1588's scalar order, with the
        // original one-byte Xfer string lengths and Money.cpp version1.
        let mut expected = vec![4];
        let int = |out: &mut Vec<u8>, value: i32| out.extend_from_slice(&value.to_le_bytes());
        int(&mut expected, 0x12345678);
        int(&mut expected, 77);
        expected.extend_from_slice(&[1, 0, 1]);
        int(&mut expected, -17);
        int(&mut expected, 8);
        int(&mut expected, 5);
        expected.extend_from_slice(&[2, 65, 0, 0xa9, 3, 1, 0]);
        for v in [3, 2, 6, -1, 4, -1, -1] {
            int(&mut expected, v);
        }
        for _ in 1..8 {
            int(&mut expected, 0);
            expected.extend_from_slice(&[0, 0, 0]);
            for _ in 0..7 {
                int(&mut expected, 0);
            }
        }
        expected.extend_from_slice(&0x01020304u32.to_le_bytes());
        expected.extend_from_slice(&[3, b'M', b'a', b'p']);
        for v in [91, 123, -2, -3] {
            int(&mut expected, v);
        }
        expected.extend_from_slice(&1u16.to_le_bytes());
        expected.push(1);
        expected.extend_from_slice(&0xf1234567u32.to_le_bytes());
        assert_eq!(info.encode().unwrap(), expected);
        assert_eq!(
            info.encode_xfer_bytes(),
            expected,
            "legacy System adapter delegates the same field walk"
        );
        assert_eq!(SkirmishGameInfo::decode(&expected).unwrap(), info);
    }
    #[test]
    fn source_v4_rejects_changed_slot_count_truncation_and_trailing_bytes() {
        let bytes = SkirmishGameInfo::default().encode().unwrap();
        for count in [-1i32, 0, 7, 9, i32::MAX] {
            let mut bad = bytes.clone();
            bad[16..20].copy_from_slice(&count.to_le_bytes());
            assert!(SkirmishGameInfo::decode(&bad).is_err());
        }
        for end in 0..bytes.len() {
            assert!(SkirmishGameInfo::decode(&bytes[..end]).is_err());
        }
        let mut trailing = bytes;
        trailing.push(0);
        assert!(SkirmishGameInfo::decode(&trailing).is_err());
    }
}
