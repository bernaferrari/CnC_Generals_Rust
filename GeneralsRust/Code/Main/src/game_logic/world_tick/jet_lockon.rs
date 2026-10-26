//! JetAIUpdate's genuine auxiliary Drawable at immediate receiving source phases.
use super::super::*;

impl GameLogic {
    /// C++ addTargeter builds/positions only on the first accepted targeter.
    pub(crate) fn admit_jet_targeter_with_effects(
        &mut self,
        jet: ObjectId,
        targeter: ObjectId,
        add: bool,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let now = self.frame;
        let born = if let Some(object) = self.objects.get_mut(&jet) {
            // C++ invokes build on accepted first admission even if unsigned
            // now+lockonTime is zero. Deadline value is not an event receipt.
            let born = add
                && object.jet_lockon_time_frames() != 0
                && object.jet_ai.untargetable_expire_frame == 0
                && object.jet_ai.targeted_by.is_empty();
            object.add_jet_targeter(targeter, add, now);
            if born {
                object.refresh_jet_lockon_position(now);
            }
            born
        } else {
            false
        };
        if born {
            self.sync_jet_lockon_drawable_with_effects(jet, effects, false, true)?;
        }
        Ok(())
    }

    /// A callback may retire/reuse either ID. Continue only the exact observed
    /// pair, including queued-dead objects still registered under that birth.
    pub(crate) fn admit_current_jet_targeter_with_effects(
        &mut self,
        jet: ObjectId,
        targeter: ObjectId,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        let Some((jet_birth, targeter_birth)) = self
            .objects
            .get(&jet)
            .zip(self.objects.get(&targeter))
            .map(|(jet, targeter)| {
                (
                    std::sync::Arc::clone(&jet.visual_draw_identity_token),
                    std::sync::Arc::clone(&targeter.visual_draw_identity_token),
                )
            })
        else {
            return Ok(false);
        };
        self.admit_jet_targeter_with_effects(jet, targeter, true, effects)?;
        Ok(self
            .objects
            .get(&jet)
            .is_some_and(|jet| std::sync::Arc::ptr_eq(&jet.visual_draw_identity_token, &jet_birth))
            && self.objects.get(&targeter).is_some_and(|targeter| {
                std::sync::Arc::ptr_eq(&targeter.visual_draw_identity_token, &targeter_birth)
            }))
    }

    #[cfg(feature = "game_client")]
    fn prepare_jet_lockon_source(
        &self,
        cursor: &str,
    ) -> Result<Option<NativeAuxiliarySource>, String> {
        let Some(template) = self.templates.get(cursor) else {
            return Ok(None);
        };
        let Some(content) = self.receiving_module_content.as_ref() else {
            return Err("native lock-on requires receiving module content".into());
        };
        // findTemplate failure is original NoDraw, never a retail-file fallback.
        let Some(plan) = content.admitted_template(cursor) else {
            return Ok(None);
        };
        let draws = template
            .receiving_draw_models()
            .ok_or_else(|| format!("lock-on '{cursor}' lacks receiving Draw model declarations"))?;
        Ok(Some(NativeAuxiliarySource {
            world: std::sync::Arc::clone(&self.live_draw_playback_world),
            epoch: self.visual_world_epoch,
            template: plan,
            draws,
            scale: template.asset_scale,
            frame: self.frame,
            birth: self.native_drawable_birth_context(),
        }))
    }

    pub(crate) fn sync_jet_lockon_drawable_with_effects(
        &mut self,
        jet_id: ObjectId,
        effects: &mut ScriptPresentationEffects<'_>,
        tick_sound: bool,
        build_if_absent: bool,
    ) -> gamelogic::GameLogicResult<bool> {
        #[cfg(not(feature = "game_client"))]
        {
            let _ = (jet_id, effects, tick_sound, build_if_absent);
            return Ok(false);
        }
        #[cfg(feature = "game_client")]
        {
            use gamelogic::GameLogicError::Configuration;
            let Some(jet) = self.objects.get(&jet_id) else {
                return Ok(false);
            };
            let birth = std::sync::Arc::clone(&jet.visual_draw_identity_token);
            let actor = self.live_draw_playback_identity(jet);
            let existing = jet.jet_ai.lockon_runtime.clone();
            if existing
                .as_ref()
                .is_some_and(|runtime| runtime.actor != actor)
            {
                return Err(Configuration(
                    "jet retained another Object birth's lock-on receipt".into(),
                ));
            }
            let cursor = jet
                .jet_ai
                .lockon_drawable_template
                .as_deref()
                .or_else(|| jet.jet_lockon_cursor_name())
                .map(str::to_owned);
            let position = jet.jet_ai.lockon_pos;
            let owner = jet.get_position();
            let hidden = jet.jet_ai.lockon_hidden;
            if position.is_none() && !build_if_absent {
                if let Some(runtime) = existing {
                    let view = effects.view.as_deref_mut().ok_or_else(|| {
                        Configuration(
                            "live lock-on retirement requires receiving native owner".into(),
                        )
                    })?;
                    view.retire_jet_lockon(&runtime.source, &runtime.receipt)
                        .map_err(Configuration)?;
                    if let Some(jet) = self.objects.get_mut(&jet_id).filter(|jet| {
                        std::sync::Arc::ptr_eq(&jet.visual_draw_identity_token, &birth)
                    }) {
                        jet.jet_ai.lockon_runtime = None;
                        jet.jet_ai.lockon_drawable_template = None;
                    }
                }
                return Ok(false);
            }
            if effects.view.is_none() {
                return if self.requires_native_drawable_owner() {
                    Err(Configuration(
                        "lock-on source requires receiving native owner".into(),
                    ))
                } else {
                    Ok(false)
                }; // Explicit headless has no actual Drawable.
            }
            let runtime =
                if let Some(runtime) = existing {
                    runtime
                } else {
                    let Some(cursor) = cursor else {
                        return Ok(false);
                    };
                    let Some(source) = self
                        .prepare_jet_lockon_source(&cursor)
                        .map_err(Configuration)?
                    else {
                        return Ok(false);
                    };
                    let receipt = effects
                        .view
                        .as_deref_mut()
                        .unwrap()
                        .create_jet_lockon(
                            &source,
                            self.receiving_module_content
                                .as_mut()
                                .expect("validated receiving definitions")
                                .module_factory_mut(),
                        )
                        .map_err(Configuration)?;
                    let runtime = crate::game_logic::object::NativeJetLockon {
                        receipt,
                        source,
                        actor,
                    };
                    let Some(jet) = self.objects.get_mut(&jet_id).filter(|jet| {
                        std::sync::Arc::ptr_eq(&jet.visual_draw_identity_token, &birth)
                    }) else {
                        effects
                            .view
                            .as_deref_mut()
                            .unwrap()
                            .retire_jet_lockon(&runtime.source, &runtime.receipt)
                            .map_err(Configuration)?;
                        return Ok(false);
                    };
                    jet.jet_ai.lockon_drawable_template =
                        Some(runtime.source.template.name().to_owned());
                    jet.jet_ai.lockon_runtime = Some(runtime.clone());
                    runtime
                };
            if !std::sync::Arc::ptr_eq(&runtime.source.world, &self.live_draw_playback_world)
                || runtime.source.epoch != self.visual_world_epoch
            {
                return Err(Configuration(
                    "jet retained a foreign native lock-on lifetime".into(),
                ));
            }
            let Some(position) = position else {
                // JetAIUpdate::buildLockonDrawableIfNecessary calls positionLockon
                // after construction; deadline zero destroys that exact new node.
                effects
                    .view
                    .as_deref_mut()
                    .unwrap()
                    .retire_jet_lockon(&runtime.source, &runtime.receipt)
                    .map_err(Configuration)?;
                if let Some(jet) = self
                    .objects
                    .get_mut(&jet_id)
                    .filter(|jet| std::sync::Arc::ptr_eq(&jet.visual_draw_identity_token, &birth))
                {
                    jet.jet_ai.lockon_runtime = None;
                    jet.jet_ai.lockon_drawable_template = None;
                }
                return Ok(false);
            };
            let dx = owner.x - position[0];
            let dz = owner.z - position[2];
            let orientation = (dx != 0.0 || dz != 0.0).then(|| dz.atan2(dx));
            effects
                .view
                .as_deref_mut()
                .unwrap()
                .position_jet_lockon(&runtime.source, &runtime.receipt, position, orientation)
                .map_err(Configuration)?;
            if tick_sound {
                let event = crate::game_logic::host_economy_log::resolve_misc_audio_event(
                    crate::game_logic::object::JET_LOCKON_TICK_SOUND,
                );
                self.queue_audio_event(
                    crate::game_logic::AudioEventRequest::new(&event)
                        .with_object(jet_id)
                        .with_position(owner),
                );
            }
            // JetAIUpdate::positionLockon does not set a shroud controller.
            // Preserve actual DrawableInfo (including its default INVALID ID).
            effects
                .view
                .as_deref_mut()
                .unwrap()
                .set_jet_lockon_scene_flags(&runtime.source, &runtime.receipt, hidden, None)
                .map_err(Configuration)?;
            Ok(true)
        }
    }

    /// Object.cpp removes actor Drawable/team/partition before module deletion;
    /// JetAIUpdate's destructor then retires its own exact auxiliary lifetime.
    pub(crate) fn retire_jet_lockon_for_object(
        &self,
        object: &mut Object,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> Result<(), String> {
        #[cfg(feature = "game_client")]
        if let Some(runtime) = object.jet_ai.lockon_runtime.clone() {
            if runtime.actor != self.live_draw_playback_identity(object) {
                return Err(
                    "jet destructor retained another Object birth's auxiliary receipt".into(),
                );
            }
            effects
                .view
                .as_deref_mut()
                .ok_or("jet module destruction requires receiving native owner")?
                .retire_jet_lockon(&runtime.source, &runtime.receipt)?;
            object.jet_ai.lockon_runtime = None;
        }
        object.jet_ai.lockon_drawable_template = None;
        Ok(())
    }

    pub(crate) fn restore_jet_lockon_drawables_with_effects(
        &mut self,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        #[cfg(feature = "game_client")]
        {
            use gamelogic::GameLogicError::Configuration;
            let ids: Vec<_> = self
                .objects
                .registered()
                .filter_map(|(id, object)| {
                    object
                        .jet_ai
                        .lockon_drawable_template
                        .is_some()
                        .then_some(*id)
                })
                .collect();
            for id in ids {
                if effects.view.is_none() {
                    if self.requires_native_drawable_owner() {
                        return Err(Configuration(
                            "saved lock-on reconstruction requires native owner".into(),
                        ));
                    }
                    continue;
                }
                let object = self.objects.get(&id).expect("saved cursor roster");
                if object.jet_ai.lockon_runtime.is_some() {
                    return Err(Configuration(
                        "saved cursor already has a live runtime owner".into(),
                    ));
                }
                let actor = self.live_draw_playback_identity(object);
                let cursor = object.jet_ai.lockon_drawable_template.clone().unwrap();
                let Some(source) = self
                    .prepare_jet_lockon_source(&cursor)
                    .map_err(Configuration)?
                else {
                    self.objects
                        .get_mut(&id)
                        .unwrap()
                        .jet_ai
                        .lockon_drawable_template = None;
                    continue;
                };
                let receipt = effects
                    .view
                    .as_deref_mut()
                    .unwrap()
                    .create_jet_lockon(
                        &source,
                        self.receiving_module_content
                            .as_mut()
                            .unwrap()
                            .module_factory_mut(),
                    )
                    .map_err(Configuration)?;
                let runtime = crate::game_logic::object::NativeJetLockon {
                    actor: actor.clone(),
                    source,
                    receipt,
                };
                let current_actor = self
                    .objects
                    .get(&id)
                    .map(|object| self.live_draw_playback_identity(object));
                let Some(object) = self.objects.get_mut(&id) else {
                    effects
                        .view
                        .as_deref_mut()
                        .unwrap()
                        .retire_jet_lockon(&runtime.source, &runtime.receipt)
                        .map_err(Configuration)?;
                    continue;
                };
                if current_actor.as_ref() != Some(&actor) {
                    effects
                        .view
                        .as_deref_mut()
                        .unwrap()
                        .retire_jet_lockon(&runtime.source, &runtime.receipt)
                        .map_err(Configuration)?;
                    return Err(Configuration(
                        "saved cursor actor changed during construction".into(),
                    ));
                }
                let position = object.jet_ai.lockon_pos;
                let hidden = object.jet_ai.lockon_hidden;
                let owner = object.get_position();
                object.jet_ai.lockon_runtime = Some(runtime.clone());
                if let Some(position) = position {
                    let dx = owner.x - position[0];
                    let dz = owner.z - position[2];
                    effects
                        .view
                        .as_deref_mut()
                        .unwrap()
                        .position_jet_lockon(
                            &runtime.source,
                            &runtime.receipt,
                            position,
                            (dx != 0.0 || dz != 0.0).then(|| dz.atan2(dx)),
                        )
                        .map_err(Configuration)?;
                    effects
                        .view
                        .as_deref_mut()
                        .unwrap()
                        .set_jet_lockon_scene_flags(&runtime.source, &runtime.receipt, hidden, None)
                        .map_err(Configuration)?;
                }
            }
        }
        Ok(())
    }

    #[cfg(feature = "game_client")]
    pub(crate) fn owned_jet_lockon_receipts(
        &self,
    ) -> Vec<game_client::core::game_client::NativeAuxiliaryReceipt> {
        self.objects
            .registered()
            .filter_map(|(_, object)| {
                object
                    .jet_ai
                    .lockon_runtime
                    .as_ref()
                    .map(|runtime| runtime.receipt.clone())
            })
            .collect()
    }
}
