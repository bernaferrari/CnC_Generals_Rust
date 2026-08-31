pub struct LogicXferCrc {
    identifier: String,
    options: u32,
    crc: u32,
}

impl LogicXferCrc {
    pub fn new() -> Self {
        Self {
            identifier: String::new(),
            options: 0,
            crc: 0,
        }
    }

    pub fn get_crc(&self) -> u32 {
        self.crc.to_be()
    }

    fn update_crc(&mut self, data: &[u8]) {
        self.crc = game_engine::common::system::xfer_crc::fold_crc_bytes(self.crc, data);
    }
}

impl Default for LogicXferCrc {
    fn default() -> Self {
        Self::new()
    }
}

impl Xfer for LogicXferCrc {
    fn get_xfer_mode(&self) -> XferMode {
        XferMode::Crc
    }

    fn get_identifier(&self) -> &str {
        &self.identifier
    }

    fn set_options(&mut self, options: u32) {
        self.options |= options;
    }

    fn clear_options(&mut self, options: u32) {
        self.options &= !options;
    }

    fn get_options(&self) -> u32 {
        self.options
    }

    fn open(&mut self, identifier: String) -> Result<(), XferStatus> {
        self.identifier = identifier;
        // C++ XferCRC::open resets the accumulator.
        self.crc = 0;
        Ok(())
    }

    fn close(&mut self) -> Result<(), XferStatus> {
        self.identifier.clear();
        Ok(())
    }

    fn begin_block(&mut self) -> Result<game_engine::system::XferBlockSize, XferStatus> {
        Ok(0)
    }

    fn end_block(&mut self) -> Result<(), XferStatus> {
        Ok(())
    }

    fn skip(&mut self, _data_size: i32) -> Result<(), XferStatus> {
        Ok(())
    }

    fn xfer_snapshot(
        &mut self,
        snapshot: &mut dyn XferSnapshotTrait,
    ) -> Result<(), XferStatus> {
        snapshot.crc(self)
    }

    fn xfer_ascii_string(&mut self, ascii_string_data: &mut String) -> Result<(), XferStatus> {
        // C++ Xfer::xferAsciiString CRCs the bytes only (no length prefix).
        self.update_crc(ascii_string_data.as_bytes());
        Ok(())
    }

    fn xfer_unicode_string(
        &mut self,
        unicode_string_data: &mut String,
    ) -> Result<(), XferStatus> {
        let bytes: Vec<u8> = unicode_string_data
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        self.update_crc(&bytes);
        Ok(())
    }

    // SAFETY: trait contract: the caller guarantees `data` is valid for
    // `data_size` bytes; it is only read here (CRC fold), never written or
    // retained.
    unsafe fn xfer_implementation(
        &mut self,
        data: *mut u8,
        data_size: usize,
    ) -> Result<(), XferStatus> {
        if data_size > 0 && !data.is_null() {
            self.update_crc(std::slice::from_raw_parts(data, data_size));
        }
        Ok(())
    }
}

impl game_engine::common::system::xfer::Xfer for LogicXferCrc {
    fn get_xfer_mode(&self) -> game_engine::common::system::xfer::XferMode {
        game_engine::common::system::xfer::XferMode::Crc
    }

    fn get_identifier(&self) -> &str {
        &self.identifier
    }

    fn set_options(&mut self, options: u32) {
        self.options |= options;
    }

    fn clear_options(&mut self, options: u32) {
        self.options &= !options;
    }

    fn get_options(&self) -> u32 {
        self.options
    }

    fn open(&mut self, identifier: &str) -> Result<(), game_engine::common::system::xfer::XferStatus> {
        self.identifier = identifier.to_string();
        self.crc = 0;
        Ok(())
    }

    fn close(&mut self) -> Result<(), game_engine::common::system::xfer::XferStatus> {
        self.identifier.clear();
        Ok(())
    }

    fn begin_block(
        &mut self,
    ) -> Result<game_engine::common::system::xfer::XferBlockSize, game_engine::common::system::xfer::XferStatus>
    {
        Ok(0)
    }

    fn end_block(&mut self) -> Result<(), game_engine::common::system::xfer::XferStatus> {
        Ok(())
    }

    fn skip(&mut self, _data_size: i32) -> Result<(), game_engine::common::system::xfer::XferStatus> {
        Ok(())
    }

    fn xfer_snapshot(
        &mut self,
        snapshot: &mut dyn game_engine::common::system::snapshot::Snapshotable,
    ) -> Result<(), game_engine::common::system::xfer::XferStatus> {
        snapshot
            .crc(self)
            .map_err(|_| game_engine::common::system::xfer::XferStatus::InvalidData)
    }

    fn xfer_ascii_string(&mut self, ascii_string_data: &mut String) -> std::io::Result<()> {
        // C++ Xfer::xferAsciiString CRCs the bytes only (no length prefix).
        self.update_crc(ascii_string_data.as_bytes());
        Ok(())
    }

    fn xfer_unicode_string(&mut self, unicode_string_data: &mut String) -> std::io::Result<()> {
        let bytes: Vec<u8> = unicode_string_data
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        self.update_crc(&bytes);
        Ok(())
    }

    // SAFETY: trait contract: the caller guarantees `data` is valid for
    // `data_size` bytes; it is only read here (CRC fold), never written or
    // retained.
    unsafe fn xfer_implementation(
        &mut self,
        data: *mut u8,
        data_size: usize,
    ) -> std::io::Result<()> {
        if data_size > 0 && !data.is_null() {
            self.update_crc(std::slice::from_raw_parts(data, data_size));
        }
        Ok(())
    }
}
