//! Presentation-clock capability for one receiving operation.
//!
//! C++ GameClient owns a clock distinct from GameLogic. A headless operation
//! explicitly disables presentation; an absent capability is an admission error.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientFrameContext {
    Frame(u32),
    NoPresentation,
}

impl ClientFrameContext {
    /// `None` means explicitly disabled presentation, never an unknown clock.
    pub fn presentation_frame(self) -> Option<u32> {
        match self {
            Self::Frame(frame) => Some(frame),
            Self::NoPresentation => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::system::xfer::{Xfer, XferStatus};
    use crate::common::system::xfer_load::XferLoad;
    use std::io::Cursor;

    #[test]
    fn receiving_xfer_clock_is_explicit_phase_local_and_never_written_into_payload() {
        let mut a = XferLoad::new(Cursor::new([7, 0, 0, 0]), 1);
        let mut b = XferLoad::new(Cursor::new([9, 0, 0, 0]), 1);
        assert_eq!(a.client_frame_context(), Err(XferStatus::InvalidData));
        a.set_client_frame_context(ClientFrameContext::Frame(37))
            .unwrap();
        b.set_client_frame_context(ClientFrameContext::Frame(900))
            .unwrap();
        assert_eq!(a.client_frame_context(), Ok(ClientFrameContext::Frame(37)));
        a.set_client_frame_context(ClientFrameContext::Frame(91))
            .unwrap();
        assert_eq!(b.client_frame_context(), Ok(ClientFrameContext::Frame(900)));
        let mut value = 0;
        a.xfer_unsigned_int(&mut value).unwrap();
        assert_eq!(value, 7, "clock context adds no bytes to the native stream");
        a.set_client_frame_context(ClientFrameContext::NoPresentation)
            .unwrap();
        assert_eq!(
            a.client_frame_context(),
            Ok(ClientFrameContext::NoPresentation)
        );
        assert_eq!(
            ClientFrameContext::NoPresentation.presentation_frame(),
            None
        );
        assert_eq!(ClientFrameContext::Frame(0).presentation_frame(), Some(0));
    }
    #[test]
    fn file_xfer_owner_also_distinguishes_unknown_headless_and_actual_client_frame() {
        use crate::System::{
            Xfer as FileXfer, XferLoad as FileXferLoad, XferStatus as FileXferStatus,
        };
        let mut receiving = FileXferLoad::new();
        let mut peer = FileXferLoad::new();
        assert_eq!(
            receiving.client_frame_context(),
            Err(FileXferStatus::InvalidData)
        );
        receiving
            .set_client_frame_context(ClientFrameContext::Frame(37))
            .unwrap();
        peer.set_client_frame_context(ClientFrameContext::Frame(900))
            .unwrap();
        receiving
            .set_client_frame_context(ClientFrameContext::Frame(91))
            .unwrap();
        assert_eq!(
            receiving.client_frame_context(),
            Ok(ClientFrameContext::Frame(91))
        );
        assert_eq!(
            peer.client_frame_context(),
            Ok(ClientFrameContext::Frame(900))
        );
        receiving
            .set_client_frame_context(ClientFrameContext::NoPresentation)
            .unwrap();
        assert_eq!(
            receiving.client_frame_context(),
            Ok(ClientFrameContext::NoPresentation)
        );
    }

    #[test]
    fn receiving_stream_clock_expires_at_operation_boundaries() {
        let mut receiving = XferLoad::new(Cursor::new([0; 4]), 1);
        receiving
            .set_client_frame_context(ClientFrameContext::Frame(37))
            .unwrap();
        receiving.close().unwrap();
        assert_eq!(
            receiving.client_frame_context(),
            Err(XferStatus::InvalidData)
        );
        receiving
            .set_client_frame_context(ClientFrameContext::NoPresentation)
            .unwrap();
        receiving.open("another-operation").unwrap();
        assert_eq!(
            receiving.client_frame_context(),
            Err(XferStatus::InvalidData)
        );
        receiving
            .set_client_frame_context(ClientFrameContext::Frame(0))
            .unwrap();
        assert_eq!(
            receiving.client_frame_context(),
            Ok(ClientFrameContext::Frame(0))
        );
    }

    #[test]
    fn receiving_file_clock_cannot_leak_through_reopen_or_failed_open() {
        use crate::System::{Xfer as FileXfer, XferLoad as FileLoad, XferStatus as FileStatus};
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("client.xfer");
        std::fs::write(&path, 7u32.to_le_bytes()).unwrap();
        let mut receiving = FileLoad::new();
        receiving
            .set_client_frame_context(ClientFrameContext::Frame(37))
            .unwrap();
        receiving.open(path.to_string_lossy().into_owned()).unwrap();
        assert_eq!(
            receiving.client_frame_context(),
            Err(FileStatus::InvalidData)
        );
        receiving
            .set_client_frame_context(ClientFrameContext::Frame(91))
            .unwrap();
        assert_eq!(
            receiving.open(path.to_string_lossy().into_owned()),
            Err(FileStatus::FileAlreadyOpen)
        );
        assert_eq!(
            receiving.client_frame_context(),
            Ok(ClientFrameContext::Frame(91)),
            "a refused double-open leaves the current operation intact"
        );
        receiving.close().unwrap();
        assert_eq!(
            receiving.client_frame_context(),
            Err(FileStatus::InvalidData)
        );
        receiving
            .set_client_frame_context(ClientFrameContext::NoPresentation)
            .unwrap();
        let missing = directory.path().join("missing.xfer");
        assert_eq!(
            receiving.open(missing.to_string_lossy().into_owned()),
            Err(FileStatus::FileNotFound)
        );
        assert_eq!(
            receiving.client_frame_context(),
            Err(FileStatus::InvalidData)
        );
        receiving.open(path.to_string_lossy().into_owned()).unwrap();
        receiving
            .set_client_frame_context(ClientFrameContext::Frame(0))
            .unwrap();
        let mut value = 0;
        receiving.xfer_unsigned_int(&mut value).unwrap();
        assert_eq!(value, 7, "operation metadata never changes stream bytes");
        receiving.close().unwrap();
    }

    #[test]
    fn deep_crc_load_forwards_the_receiving_operation_clock() {
        use crate::common::system::xfer_crc::XferDeepCRC;
        let mut receiving = XferDeepCRC::new(XferLoad::new(Cursor::new(7u32.to_le_bytes()), 1));
        assert_eq!(
            receiving.client_frame_context(),
            Err(XferStatus::InvalidData)
        );
        receiving
            .set_client_frame_context(ClientFrameContext::Frame(37))
            .unwrap();
        assert_eq!(
            receiving.client_frame_context(),
            Ok(ClientFrameContext::Frame(37))
        );
        let before = receiving.get_global_crc();
        receiving
            .set_client_frame_context(ClientFrameContext::NoPresentation)
            .unwrap();
        assert_eq!(
            receiving.get_global_crc(),
            before,
            "clock metadata is not checksummed stream data"
        );
        receiving.open("receiving-operation").unwrap();
        assert_eq!(
            receiving.client_frame_context(),
            Err(XferStatus::InvalidData)
        );
        receiving
            .set_client_frame_context(ClientFrameContext::Frame(0))
            .unwrap();
        let mut value = 0;
        receiving.xfer_unsigned_int(&mut value).unwrap();
        assert_eq!(value, 7);
        receiving.close().unwrap();
        assert_eq!(
            receiving.client_frame_context(),
            Err(XferStatus::InvalidData)
        );
    }
}
