// CRC implementation matching C++ Generals CRC algorithm
//
// This is a custom shift-and-add CRC algorithm used by the C++ implementation.
// The algorithm is NOT a standard CRC32, but a proprietary EA implementation.

/// CRC calculator matching the C++ implementation exactly
#[derive(Debug, Clone, Copy, Default)]
pub struct Crc {
    crc: u32,
}

impl Crc {
    /// Create a new CRC calculator with zero initial value
    pub fn new() -> Self {
        Self { crc: 0 }
    }

    /// Compute CRC for a buffer, adding to the current CRC value
    ///
    /// This matches the C++ implementation:
    /// ```c++
    /// void CRC::computeCRC(const void *buf, Int len) {
    ///     UnsignedByte *uintPtr = (UnsignedByte *)buf;
    ///     for (int i=0 ; i<len ; i++) {
    ///         addCRC(*(uintPtr++));
    ///     }
    /// }
    ///
    /// void CRC::addCRC(UnsignedByte val) {
    ///     int hibit = if (crc & 0x80000000) { 1 } else { 0 };
    ///     crc <<= 1;
    ///     crc += val;
    ///     crc += hibit;
    /// }
    /// ```
    pub fn compute(&mut self, buf: &[u8]) {
        for &byte in buf {
            self.add_crc(byte);
        }
    }

    /// Add a single byte to the CRC calculation
    #[inline]
    fn add_crc(&mut self, val: u8) {
        let hibit = if self.crc & 0x8000_0000 != 0 { 1 } else { 0 };
        self.crc <<= 1;
        self.crc = self.crc.wrapping_add(val as u32);
        self.crc = self.crc.wrapping_add(hibit);
    }

    /// Clear the CRC to zero
    pub fn clear(&mut self) {
        self.crc = 0;
    }

    /// Get the current CRC value
    pub fn get(&self) -> u32 {
        self.crc
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crc_empty_buffer() {
        let mut crc = Crc::new();
        crc.compute(&[]);
        assert_eq!(crc.get(), 0);
    }

    #[test]
    fn test_crc_single_byte() {
        let mut crc = Crc::new();
        crc.compute(&[0x42]);
        // Expected: shift left (0 << 1 = 0), add 0x42 (0 + 0x42 = 0x42), add hibit (0x42 + 0 = 0x42)
        assert_eq!(crc.get(), 0x42);
    }

    #[test]
    fn test_crc_two_bytes() {
        let mut crc = Crc::new();
        crc.compute(&[0x01, 0x02]);
        // First byte: 0 << 1 = 0, 0 + 1 = 1, 1 + 0 = 1
        // Second byte: 1 << 1 = 2, 2 + 2 = 4, 4 + 0 = 4
        assert_eq!(crc.get(), 0x04);
    }

    #[test]
    fn test_crc_with_hibit() {
        let mut crc = Crc::new();
        crc.crc = 0x8000_0000;
        crc.add_crc(0x01);
        // hibit = 1, crc << 1 = 0, 0 + 1 = 1, 1 + 1 = 2
        assert_eq!(crc.get(), 0x02);
    }

    #[test]
    fn test_crc_clear() {
        let mut crc = Crc::new();
        crc.compute(&[0xFF, 0xFF]);
        assert_ne!(crc.get(), 0);
        crc.clear();
        assert_eq!(crc.get(), 0);
    }

    #[test]
    fn test_crc_deterministic() {
        let data = b"Hello, World!";

        let mut crc1 = Crc::new();
        crc1.compute(data);

        let mut crc2 = Crc::new();
        crc2.compute(data);

        assert_eq!(crc1.get(), crc2.get());
    }

    #[test]
    fn test_crc_incremental() {
        let data = b"Hello, World!";

        let mut crc1 = Crc::new();
        crc1.compute(data);

        let mut crc2 = Crc::new();
        crc2.compute(&data[..6]);
        crc2.compute(&data[6..]);

        assert_eq!(crc1.get(), crc2.get());
    }
}
