//! Handle abstractions mirroring the original Miles handles used by WWAudio.

pub mod base_handle;
pub mod listener_handle;
pub mod sound2d_handle;
pub mod sound3d_handle;
pub mod soundstream_handle;

pub use base_handle::BaseSoundHandle;
pub use listener_handle::ListenerHandle;
pub use sound2d_handle::Sound2DHandle;
pub use sound3d_handle::Sound3DHandle;
pub use soundstream_handle::SoundStreamHandle;
