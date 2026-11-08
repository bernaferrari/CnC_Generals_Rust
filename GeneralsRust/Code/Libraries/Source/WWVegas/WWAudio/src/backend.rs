//! Audio backend descriptors.
//!
//! The 3D provider table retains legacy Miles identifiers for compatibility.
//! Audio is mixed in software and sent to the platform through CPAL; selecting
//! a legacy provider does not enable a separate hardware backend.

use crate::{Driver2DKind, formats::AudioFormat, wwaudio::DriverType3D};

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

/// Registry of legacy 3D provider labels and the CPAL-backed 2D driver.
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

        let drivers = vec![
            DriverDescriptor {
                name: "Software Mixer (CPAL)",
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

    /// Enumerate the available 2D output implementation.
    pub fn drivers(&self) -> &[DriverDescriptor] {
        &self.drivers
    }

    pub fn find_provider(&self, provider_type: DriverType3D) -> Option<&BackendDescriptor> {
        self.providers
            .iter()
            .find(|descriptor| descriptor.provider_type == provider_type)
    }
}
