use crate::{
    handles::{BaseSoundHandle, ListenerHandle, Sound2DHandle, Sound3DHandle, SoundStreamHandle},
    save_load::SavedSoundRecord,
    sound3d::Sound3D,
    sound_pseudo3d::SoundPseudo3D,
    sound_scene::SceneSound,
    sound_scene_obj::SoundObjectId,
    AudioChannel, AudioSource,
};
use std::sync::Arc;

#[derive(Clone)]
pub enum WWHandle {
    Base(BaseSoundHandle),
    Sound2D {
        id: SoundObjectId,
        handle: Sound2DHandle,
    },
    Sound3D {
        id: SoundObjectId,
        handle: Sound3DHandle,
    },
    SoundStream {
        id: SoundObjectId,
        handle: SoundStreamHandle,
    },
    Listener {
        id: SoundObjectId,
        handle: ListenerHandle,
    },
}

impl WWHandle {
    pub fn id(&self) -> Option<SoundObjectId> {
        match self {
            Self::Sound2D { id, .. }
            | Self::Sound3D { id, .. }
            | Self::SoundStream { id, .. }
            | Self::Listener { id, .. } => Some(*id),
            Self::Base(_) => None,
        }
    }

    pub fn stop(&self) {
        match self {
            Self::Sound2D { handle, .. } => {
                let _ = handle.stop_sample();
            }
            Self::Sound3D { handle, .. } => {
                let _ = handle.stop_sample();
            }
            Self::SoundStream { handle, .. } => {
                let _ = handle.stop_sample();
            }
            Self::Listener { .. } | Self::Base(_) => {}
        }
    }

    pub fn set_miles_handle(&mut self, id: u32) {
        match self {
            Self::Base(base) => base.set_miles_handle(id),
            Self::Sound2D { handle, .. } => handle.base.set_miles_handle(id),
            Self::Sound3D { handle, .. } => {
                handle.base.set_miles_handle(id);
                handle.sample_handle().base.set_miles_handle(id);
            }
            Self::SoundStream { handle, .. } => {
                handle.base.set_miles_handle(id);
                handle.sample_handle().base.set_miles_handle(id);
            }
            Self::Listener { .. } => {}
        }
    }

    pub fn miles_handle(&self) -> Option<u32> {
        match self {
            Self::Base(base) => base.miles_handle(),
            Self::Sound2D { handle, .. } => handle.base.miles_handle(),
            Self::Sound3D { handle, .. } => handle.base.miles_handle(),
            Self::SoundStream { handle, .. } => handle.base.miles_handle(),
            Self::Listener { .. } => None,
        }
    }

    pub fn start(&self) -> bool {
        match self {
            Self::Sound2D { handle, .. } => handle.start_sample().is_ok(),
            Self::Sound3D { handle, .. } => handle.start_sample().is_ok(),
            Self::SoundStream { handle, .. } => handle.start_sample().is_ok(),
            _ => false,
        }
    }

    pub fn as_sound2d(&self) -> Option<&Sound2DHandle> {
        match self {
            Self::Sound2D { handle, .. } => Some(handle),
            _ => None,
        }
    }

    pub fn as_sound3d(&self) -> Option<&Sound3DHandle> {
        match self {
            Self::Sound3D { handle, .. } => Some(handle),
            _ => None,
        }
    }

    pub fn as_stream(&self) -> Option<&SoundStreamHandle> {
        match self {
            Self::SoundStream { handle, .. } => Some(handle),
            _ => None,
        }
    }
}

pub fn make_2d_handle(
    id: SoundObjectId,
    channel: AudioChannel,
    source: Arc<AudioSource>,
) -> WWHandle {
    let mut handle = Sound2DHandle::new(channel);
    handle.initialize(source);
    WWHandle::Sound2D { id, handle }
}

pub fn make_3d_handle(
    id: SoundObjectId,
    sound: Sound3D,
    sample_handle: Sound2DHandle,
    source: Arc<AudioSource>,
) -> WWHandle {
    let mut handle = Sound3DHandle::new(sound, sample_handle);
    handle.base.initialize(source);
    WWHandle::Sound3D { id, handle }
}

pub fn make_stream_handle(
    id: SoundObjectId,
    channel: AudioChannel,
    source: Arc<AudioSource>,
) -> WWHandle {
    let mut handle = SoundStreamHandle::new(channel);
    handle.initialize(source);
    WWHandle::SoundStream { id, handle }
}

pub fn make_listener_handle(id: SoundObjectId, listener: ListenerHandle) -> WWHandle {
    WWHandle::Listener {
        id,
        handle: listener,
    }
}

pub fn instantiate_scene_sound(record: &SavedSoundRecord) -> SceneSound {
    match record.class_id {
        crate::SoundClassId::ThreeD => {
            let mut sound = Sound3D::new(record.id);
            sound.set_position(record.position);
            sound.set_dropoff_radius(record.dropoff_radius);
            sound.as_audible_mut().set_priority_scalar(record.priority);
            SceneSound::Sound3D(sound)
        }
        crate::SoundClassId::Pseudo3D => {
            let mut pseudo = SoundPseudo3D::new(record.id);
            pseudo.base.set_position(record.position);
            pseudo.base.set_dropoff_radius(record.dropoff_radius);
            pseudo
                .base
                .as_audible_mut()
                .set_priority_scalar(record.priority);
            SceneSound::Pseudo3D(pseudo)
        }
        _ => {
            let mut audible = crate::AudibleSound::new(record.id, record.class_id);
            audible.set_position(record.position);
            audible.base.set_priority(record.priority);
            SceneSound::Audible(audible)
        }
    }
}
