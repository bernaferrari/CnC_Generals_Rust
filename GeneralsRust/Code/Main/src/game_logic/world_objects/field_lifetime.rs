//! Receiving field expiry, distinct C++ DeletionUpdate and LifetimeUpdate.
//! Existing field call sites retain their phase; callbacks run while installed.
use super::super::*;
use crate::game_logic::host_lifetime_update::HostLifetimeUpdateData;

#[derive(Clone, Copy)]
enum FieldLifetime {
    Fire,
    Toxin,
    Radiation,
}

impl FieldLifetime {
    fn projected_deadline(self, object: &Object) -> Option<u32> {
        match self {
            Self::Fire if object.inferno_fire_field => object.inferno_fire_field_expires_frame,
            Self::Toxin if object.anthrax_toxin_field => object.anthrax_toxin_field_expires_frame,
            Self::Radiation if object.nuke_radiation_field => {
                object.nuke_radiation_field_expires_frame
            }
            _ => None,
        }
    }

    fn deadline(self, object: &Object) -> Option<u32> {
        let projected = self.projected_deadline(object)?;
        match self {
            Self::Fire => Some(projected),
            // Retain the old serialized field timestamp as a projection. Only
            // the receiving LifetimeUpdate determines its active deadline.
            Self::Toxin | Self::Radiation => Some(
                object
                    .lifetime_update
                    .as_ref()
                    .map_or(projected, |lifetime| lifetime.expire_at_frame),
            ),
        }
    }

    fn consume_projection(self, object: &mut Object) {
        match self {
            Self::Fire => object.inferno_fire_field_expires_frame = None,
            Self::Toxin => object.anthrax_toxin_field_expires_frame = None,
            Self::Radiation => object.nuke_radiation_field_expires_frame = None,
        }
    }
}

impl GameLogic {
    pub fn update_inferno_fire_field_objects(&mut self) {
        self.expire_field_lifetimes(FieldLifetime::Fire);
    }

    pub fn update_anthrax_toxin_field_objects(&mut self) {
        self.expire_field_lifetimes(FieldLifetime::Toxin);
    }

    pub fn update_nuke_radiation_field_objects(&mut self) {
        self.expire_field_lifetimes(FieldLifetime::Radiation);
    }

    fn expire_field_lifetimes(&mut self, kind: FieldLifetime) {
        let frame = self.frame;
        let due: Vec<_> = self
            .objects
            .iter()
            .filter_map(|(id, object)| {
                kind.deadline(object)
                    .filter(|deadline| *deadline <= frame)
                    .map(|deadline| (*id, deadline))
            })
            .collect();
        for (id, deadline) in due {
            let Some(object) = self.objects.get_mut(&id) else {
                continue;
            };
            kind.consume_projection(object);
            let deletion = matches!(kind, FieldLifetime::Fire);
            let admitted = if deletion {
                true
            } else {
                // Old saves can carry only the field deadline. Admit it once
                // into the same receiving module, never a second live timer.
                object
                    .lifetime_update
                    .get_or_insert(HostLifetimeUpdateData {
                        expire_at_frame: deadline,
                        active: true,
                    });
                // Consume before callbacks; a rejected kill stays asleep and
                // the later generic AI update cannot repeat or rearm it.
                object.tick_lifetime_update(frame)
            };
            if !admitted {
                continue;
            }
            // End the Object loan before immediate onDelete/onDie callbacks.
            if deletion {
                // CPP DeletionUpdate.cpp:82: direct destruction bypasses Body.
                self.destroy_object(id);
            } else {
                // CPP LifetimeUpdate.cpp:70 / Object.h:206 default kill input.
                self.apply_owned_kill(
                    id,
                    crate::game_logic::combat::DamageType::Unresistable,
                    crate::game_logic::host_usa_pilot::HostDeathType::Normal,
                );
            }
        }
    }
}
