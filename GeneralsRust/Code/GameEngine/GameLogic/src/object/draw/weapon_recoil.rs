//! The per-barrel state advanced by W3DModelDraw::handleClientRecoil.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum BarrelRecoilPhase {
    #[default]
    Idle = 0,
    RecoilStart = 1,
    Recoil = 2,
    Settle = 3,
}

impl BarrelRecoilPhase {
    pub fn from_wire(value: i32) -> Option<Self> {
        Some(match value {
            0 => Self::Idle,
            1 => Self::RecoilStart,
            2 => Self::Recoil,
            3 => Self::Settle,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct BarrelRecoil {
    pub phase: BarrelRecoilPhase,
    pub shift: f32,
    pub recoil_rate: f32,
}

impl BarrelRecoil {
    pub fn start(&mut self, initial_rate: f32) {
        self.phase = BarrelRecoilPhase::RecoilStart;
        self.recoil_rate = initial_rate;
    }

    /// The Draw module samples muzzle visibility before advancing the barrel.
    pub fn muzzle_visible(&self) -> bool {
        self.phase == BarrelRecoilPhase::RecoilStart
    }

    pub fn advance(&mut self, has_recoil_bone: bool, damping: f32, max_shift: f32, settle: f32) {
        use BarrelRecoilPhase::*;

        if !has_recoil_bone {
            self.phase = Idle;
            return;
        }
        match self.phase {
            Idle => {}
            RecoilStart | Recoil => {
                self.shift += self.recoil_rate;
                self.recoil_rate *= damping;
                if self.shift >= max_shift {
                    self.shift = max_shift;
                    self.phase = Settle;
                } else if self.recoil_rate.abs() < 0.01 {
                    self.phase = Settle;
                }
            }
            Settle => {
                self.shift -= settle;
                if self.shift <= 0.0 {
                    self.shift = 0.0;
                    self.phase = Idle;
                }
            }
        }
    }
}
