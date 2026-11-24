//! WorldBuilder object ambient properties from Object.cpp.
use super::audio_event_rts::{AC_LOOP, AudioEventInfo, AudioPriority};
use super::dynamic_audio_event_info::DynamicAudioEventInfo;
use crate::common::dict::{Dict, DictType};
use crate::common::well_known_keys::*;
use std::sync::Arc;

pub enum MapAmbientSelection {
    Template,
    Named(String),
    Silence,
}

pub struct MapAmbientProperties {
    pub selection: MapAmbientSelection,
    pub enabled: Option<bool>,
    overrides: Option<AmbientOverrides>,
}

struct AmbientOverrides {
    looping: Option<bool>,
    loop_count: Option<i32>,
    volume: Option<f32>,
    min_volume: Option<f32>,
    min_range: Option<f32>,
    max_range: Option<f32>,
    priority: Option<AudioPriority>,
}

pub enum ResolvedMapAmbient {
    Silence,
    Selected {
        enabled: Option<bool>,
        custom: Option<DynamicAudioEventInfo>,
    },
}

impl MapAmbientProperties {
    pub fn from_dict(dict: &Dict) -> Self {
        let boolean =
            |key| (dict.get_type(key) == Some(DictType::Bool)).then(|| dict.get_bool(key));
        let integer = |key| (dict.get_type(key) == Some(DictType::Int)).then(|| dict.get_int(key));
        let real = |key| (dict.get_type(key) == Some(DictType::Real)).then(|| dict.get_real(key));
        let key = key_object_sound_ambient();
        let selection = if dict.get_type(key) == Some(DictType::AsciiString) {
            let name = dict.get_ascii_string(key);
            if name.is_empty() {
                MapAmbientSelection::Silence
            } else {
                MapAmbientSelection::Named(name)
            }
        } else {
            MapAmbientSelection::Template
        };
        let overrides = (boolean(key_object_sound_ambient_customized()) == Some(true)
            && !matches!(selection, MapAmbientSelection::Silence))
        .then(|| AmbientOverrides {
            looping: boolean(key_object_sound_ambient_looping()),
            loop_count: integer(key_object_sound_ambient_loop_count()),
            volume: real(key_object_sound_ambient_volume()),
            min_volume: real(key_object_sound_ambient_min_volume()),
            min_range: real(key_object_sound_ambient_min_range()),
            max_range: real(key_object_sound_ambient_max_range()),
            priority: integer(key_object_sound_ambient_priority()).map(|priority| match priority {
                0 => AudioPriority::Lowest,
                1 => AudioPriority::Low,
                2 => AudioPriority::Normal,
                3 => AudioPriority::High,
                _ => AudioPriority::Critical,
            }),
        });
        Self {
            selection,
            enabled: boolean(key_object_sound_ambient_enabled()),
            overrides,
        }
    }

    /// Resolve against the receiving catalog. The pristine template slot is
    /// the base for customization when no named definition was selected.
    pub fn resolve(
        &self,
        template: impl FnOnce() -> Option<Arc<AudioEventInfo>>,
        mut named: impl FnMut(&str) -> Option<Arc<AudioEventInfo>>,
    ) -> ResolvedMapAmbient {
        let named = match &self.selection {
            MapAmbientSelection::Silence => return ResolvedMapAmbient::Silence,
            MapAmbientSelection::Named(name) => named(name),
            MapAmbientSelection::Template => None,
        };
        let has_named_override = named.is_some();
        let base = named.or_else(template);
        let custom = base.as_deref().and_then(|base| {
            if let Some(overrides) = &self.overrides {
                let looping = overrides.looping.unwrap_or(base.control & AC_LOOP != 0);
                let loop_count = overrides.loop_count.filter(|_| looping);
                if has_named_override
                    || overrides.looping.is_some()
                    || loop_count.is_some()
                    || overrides.volume.is_some()
                    || overrides.min_volume.is_some()
                    || overrides.min_range.is_some()
                    || overrides.max_range.is_some()
                    || overrides.priority.is_some()
                {
                    let mut info = DynamicAudioEventInfo::from_base_info(base);
                    if let Some(value) = overrides.looping {
                        info.override_loop_flag(value);
                    }
                    if let Some(value) = loop_count {
                        info.override_loop_count(value);
                    }
                    if let Some(value) = overrides.min_volume {
                        info.override_min_volume(value);
                    }
                    if let Some(value) = overrides.volume {
                        info.override_volume(value);
                    }
                    if let Some(value) = overrides.min_range {
                        info.override_min_range(value);
                    }
                    if let Some(value) = overrides.max_range {
                        info.override_max_range(value);
                    }
                    if let Some(value) = overrides.priority {
                        info.override_priority(value);
                    }
                    return Some(info);
                }
            }
            has_named_override.then(|| DynamicAudioEventInfo::from_base_info(base))
        });
        let enabled = self.enabled.or_else(|| {
            custom
                .as_ref()
                .map(DynamicAudioEventInfo::get_audio_event_info)
                .or(base.as_deref())
                .map(AudioEventInfo::is_permanent_sound)
        });
        ResolvedMapAmbient::Selected { enabled, custom }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_customization_disables_looping_before_reading_count() {
        let mut dict = Dict::new();
        dict.set_bool(key_object_sound_ambient_customized(), true);
        dict.set_bool(key_object_sound_ambient_looping(), false);
        dict.set_int(key_object_sound_ambient_loop_count(), 3);
        let template = Arc::new(AudioEventInfo {
            audio_name: "TemplateLoop".into(),
            control: AC_LOOP,
            loop_count: 0,
            ..Default::default()
        });
        let ResolvedMapAmbient::Selected { enabled, custom } =
            MapAmbientProperties::from_dict(&dict).resolve(|| Some(template), |_| None)
        else {
            panic!("template override is not a silence marker");
        };
        let info = custom.unwrap();
        assert_eq!(enabled, Some(false));
        assert_eq!(info.get_audio_event_info().control & AC_LOOP, 0);
        assert_eq!(info.get_audio_event_info().loop_count, 0);
        assert!(!info.was_loop_count_overridden());
    }

    #[test]
    fn uncustomized_template_uses_permanence_and_ignores_unarmed_overrides() {
        let mut dict = Dict::new();
        dict.set_bool(key_object_sound_ambient_looping(), false);
        let template = Arc::new(AudioEventInfo {
            audio_name: "TemplateLoop".into(),
            control: AC_LOOP,
            loop_count: 0,
            ..Default::default()
        });
        let ResolvedMapAmbient::Selected { enabled, custom } =
            MapAmbientProperties::from_dict(&dict).resolve(|| Some(template), |_| None)
        else {
            panic!("template sound is not a silence marker");
        };
        assert_eq!(enabled, Some(true));
        assert!(custom.is_none());
    }
}
