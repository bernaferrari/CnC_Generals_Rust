//! No-init constructor marker mirroring WWLib `noinit.h`.

/// Marker used to signal a no-init constructor path.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoInit;
