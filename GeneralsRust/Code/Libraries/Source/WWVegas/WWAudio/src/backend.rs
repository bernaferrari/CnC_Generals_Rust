//! Audio backend descriptors mirroring the original Miles provider tables.
//!
//! This module does not yet swap concrete mixer implementations, but it
//! centralises knowledge about the legacy DirectSound/WaveOut/EAX providers so
//! the higher-level device code can report the same options the C++ version
//! exposes. As platform specific backends arrive, they can plug into this
//! registry.

use crate::{formats::AudioFormat, wwaudio::DriverType3D, Driver2DKind};

/// Enumeration of legacy backend identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    DirectSound,
    WaveOut,
    Eax,
    A3d,
    Rsx,
    Pseudo3d,
    Software,
}

/// Minimal capability flags for a backend.
#[derive(Debug, Clone, Copy)]
pub struct BackendCapabilities {
    pub supports_3d: bool,
    pub hardware_accelerated: bool,
}

/// Descriptor used to populate the 3D provider list.
#[derive(Debug, Clone, Copy)]
pub struct BackendDescriptor {
    pub name: &'static str,
    pub kind: BackendKind,
    pub provider_type: DriverType3D,
    pub capabilities: BackendCapabilities,
}

/// Descriptor used for 2D driver tables.
#[derive(Debug, Clone)]
pub struct DriverDescriptor {
    pub name: &'static str,
    pub backend: BackendKind,
    pub driver_kind: Driver2DKind,
    pub preferred_format: Option<AudioFormat>,
    pub hardware_accelerated: bool,
}

/// Registry of available audio backends mirroring the original tables.
#[derive(Debug, Clone)]
pub struct BackendManager {
    providers: Vec<BackendDescriptor>,
    drivers: Vec<DriverDescriptor>,
}

impl BackendManager {
    pub fn new(default_format: AudioFormat) -> Self {
        // These names mirror the strings from WWAudio::Build_3D_Driver_List.
        let providers = vec![
            BackendDescriptor {
                name: "Miles Fast 3D Software",
                kind: BackendKind::Software,
                provider_type: DriverType3D::D3dSound,
                capabilities: BackendCapabilities {
                    supports_3d: true,
                    hardware_accelerated: false,
                },
            },
            BackendDescriptor {
                name: "Miles Enhanced 3D (EAX Compatible)",
                kind: BackendKind::Eax,
                provider_type: DriverType3D::Eax,
                capabilities: BackendCapabilities {
                    supports_3d: true,
                    hardware_accelerated: true,
                },
            },
            BackendDescriptor {
                name: "Miles Legacy A3D Emulation",
                kind: BackendKind::A3d,
                provider_type: DriverType3D::A3d,
                capabilities: BackendCapabilities {
                    supports_3d: true,
                    hardware_accelerated: false,
                },
            },
            BackendDescriptor {
                name: "Miles RSX Positional Audio",
                kind: BackendKind::Rsx,
                provider_type: DriverType3D::Rsx,
                capabilities: BackendCapabilities {
                    supports_3d: true,
                    hardware_accelerated: false,
                },
            },
            BackendDescriptor {
                name: "Miles Pseudo-3D Mixer",
                kind: BackendKind::Pseudo3d,
                provider_type: DriverType3D::Pseudo,
                capabilities: BackendCapabilities {
                    supports_3d: false,
                    hardware_accelerated: false,
                },
            },
        ];

        // Names mirrored from the DirectSound/WaveOut driver selection.
        let drivers = vec![
            DriverDescriptor {
                name: "Miles Fast 2D DirectSound",
                backend: BackendKind::DirectSound,
                driver_kind: Driver2DKind::DirectSound,
                preferred_format: Some(default_format),
                hardware_accelerated: true,
            },
            DriverDescriptor {
                name: "Miles 2D WaveOut",
                backend: BackendKind::WaveOut,
                driver_kind: Driver2DKind::WaveOut,
                preferred_format: Some(default_format),
                hardware_accelerated: false,
            },
            DriverDescriptor {
                name: "Software Mixer (Rodio)",
                backend: BackendKind::Software,
                driver_kind: Driver2DKind::Unknown,
                preferred_format: Some(default_format),
                hardware_accelerated: false,
            },
        ];

        Self { providers, drivers }
    }

    /// Enumerate known 3D providers in priority order.
    pub fn providers(&self) -> &[BackendDescriptor] {
        &self.providers
    }

    /// Enumerate known 2D drivers.
    pub fn drivers(&self) -> &[DriverDescriptor] {
        &self.drivers
    }

    pub fn find_provider(&self, provider_type: DriverType3D) -> Option<&BackendDescriptor> {
        self.providers
            .iter()
            .find(|descriptor| descriptor.provider_type == provider_type)
    }
}
