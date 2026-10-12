//! Original Xfer.cpp upgrade-name wire with explicit immutable definitions.
use super::{Xfer, XferMode, XferVersion};
use crate::common::system::upgrade::UpgradeCenter;
use std::io;

pub(super) fn xfer_upgrade_mask_with_center<X: Xfer + ?Sized>(
    xfer: &mut X,
    mask: &mut u128,
    center: &UpgradeCenter,
) -> io::Result<()> {
    xfer_upgrade_mask_with_lookup(
        xfer,
        mask,
        &mut |bits| center.upgrade_names_in_mask(bits),
        &mut |name| center.mask_for_name(name).map(|mask| mask.bits()),
    )
}

pub(super) fn xfer_upgrade_mask_with_lookup<X: Xfer + ?Sized>(
    xfer: &mut X,
    upgrade_mask_data: &mut u128,
    selected_names: &mut dyn FnMut(u128) -> Vec<String>,
    mask_for_name: &mut dyn FnMut(&str) -> Option<u128>,
) -> io::Result<()> {
    const CURRENT_VERSION: XferVersion = 1;
    let mut version = CURRENT_VERSION;
    xfer.xfer_version(&mut version, CURRENT_VERSION)?;

    match xfer.get_xfer_mode() {
        XferMode::Save => {
            // PARITY_NOTE: C++ Xfer::xferUpgradeMask writes the name of every
            // template whose mask is set, walking TheUpgradeCenter's list.
            let selected_names = selected_names(*upgrade_mask_data);

            let mut count = selected_names.len() as u16;
            xfer.xfer_unsigned_short(&mut count)?;
            for mut upgrade_name in selected_names {
                xfer.xfer_ascii_string(&mut upgrade_name)?;
            }
            Ok(())
        }
        XferMode::Load => {
            let mut count = 0u16;
            xfer.xfer_unsigned_short(&mut count)?;
            *upgrade_mask_data = 0;

            for _ in 0..count {
                let mut upgrade_name = String::new();
                xfer.xfer_ascii_string(&mut upgrade_name)?;

                // C++: TheUpgradeCenter->findUpgrade(name), XFER_UNKNOWN_STRING if NULL.
                let Some(upgrade_mask) = mask_for_name(&upgrade_name) else {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "Xfer::xfer_upgrade_mask - Unknown upgrade '{}'",
                            upgrade_name
                        ),
                    ));
                };

                *upgrade_mask_data |= upgrade_mask;
            }
            Ok(())
        }
        XferMode::Crc => xfer.xfer_u128(upgrade_mask_data),
        _ => Err(io::Error::new(
            io::ErrorKind::Unsupported,
            format!(
                "xfer_upgrade_mask - Unknown xfer mode {:?}",
                xfer.get_xfer_mode()
            ),
        )),
    }
}

#[cfg(test)]
mod owned_tests {
    use super::*;
    use crate::common::ascii_string::AsciiString;
    use crate::common::system::{xfer_load::XferLoad, xfer_save::XferSave};
    use std::io::Cursor;

    fn save(center: &UpgradeCenter, mut mask: u128) -> Vec<u8> {
        let mut bytes = Vec::new();
        xfer_upgrade_mask_with_center(
            &mut XferSave::new(Cursor::new(&mut bytes), 0),
            &mut mask,
            center,
        )
        .unwrap();
        bytes
    }

    #[test]
    fn same_names_resolve_to_the_receiving_masks_in_original_name_order() {
        let mut source = UpgradeCenter::new();
        source.init();
        let a = source
            .new_upgrade(AsciiString::from("OwnedA"))
            .get_mask()
            .bits();
        let b = source
            .new_upgrade(AsciiString::from("OwnedB"))
            .get_mask()
            .bits();
        let bytes = save(&source, a | b);
        let mut reader = XferLoad::new(Cursor::new(&bytes), 0);
        let mut version = 1;
        reader.xfer_version(&mut version, 1).unwrap();
        let mut count = 0;
        reader.xfer_unsigned_short(&mut count).unwrap();
        assert_eq!(count, 2);
        let mut names = Vec::new();
        for _ in 0..count {
            let mut name = String::new();
            reader.xfer_ascii_string(&mut name).unwrap();
            names.push(name);
        }
        assert_eq!(
            names,
            ["OwnedB", "OwnedA"],
            "CPP head-inserted definition list"
        );
        let mut receiver = UpgradeCenter::new();
        receiver.init();
        receiver.new_upgrade(AsciiString::from("ReceivingOnly"));
        let rb = receiver
            .new_upgrade(AsciiString::from("OwnedB"))
            .get_mask()
            .bits();
        let ra = receiver
            .new_upgrade(AsciiString::from("OwnedA"))
            .get_mask()
            .bits();
        assert_ne!(a | b, ra | rb);
        let mut restored = 0;
        xfer_upgrade_mask_with_center(
            &mut XferLoad::new(Cursor::new(&bytes), 0),
            &mut restored,
            &receiver,
        )
        .unwrap();
        assert_eq!(restored, ra | rb);
        assert_eq!(
            save(&source, a | b),
            bytes,
            "receiving construction cannot alter source definitions"
        );
    }

    #[test]
    fn missing_receiving_definition_preserves_original_first_error_and_partial_mask() {
        let mut source = UpgradeCenter::new();
        let a = source
            .new_upgrade(AsciiString::from("OwnedA"))
            .get_mask()
            .bits();
        let b = source
            .new_upgrade(AsciiString::from("OwnedB"))
            .get_mask()
            .bits();
        let bytes = save(&source, a | b);
        let mut receiver = UpgradeCenter::new();
        receiver.new_upgrade(AsciiString::from("ReceivingOnly"));
        let rb = receiver
            .new_upgrade(AsciiString::from("OwnedB"))
            .get_mask()
            .bits();
        let mut restored = u128::MAX;
        let error = xfer_upgrade_mask_with_center(
            &mut XferLoad::new(Cursor::new(bytes), 0),
            &mut restored,
            &receiver,
        )
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("OwnedA"));
        assert_eq!(
            restored, rb,
            "CPP clears first, applies B, then rejects unknown A"
        );
    }
}
