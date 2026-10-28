//! Genuine objectless CPU Drawable lifetimes on the receiving native owner.
use super::*;
use game_engine::common::thing::thing_template::AdmittedReceivingTemplate;
use std::sync::{Arc, Weak};

/// A C++ Drawable pointer represented by the exact CPU lifetime, not a global
/// numeric ID. Serialization stores the authored template and recreates it.
#[derive(Clone, Debug)]
pub struct NativeAuxiliaryReceipt {
    id: DrawableId,
    generation: u64,
    owner: Weak<()>,
}
impl PartialEq for NativeAuxiliaryReceipt {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.generation == other.generation
            && Weak::ptr_eq(&self.owner, &other.owner)
    }
}
impl Eq for NativeAuxiliaryReceipt {}
impl NativeAuxiliaryReceipt {
    pub fn drawable_id(&self) -> DrawableId {
        self.id
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn owner_token(&self) -> Option<Arc<()>> {
        self.owner.upgrade()
    }
}

/// Ephemeral original Drawable-list observation, never simulation state.
#[derive(Clone, Debug)]
pub enum NativeDrawCandidate {
    Actor(PresentationDirectDrawableBindingKey),
    Aux(NativeAuxiliaryReceipt),
}
#[derive(Clone, Debug)]
pub struct NativeAuxiliaryDrawObservation {
    pub base: Matrix4,
    pub instance: Matrix4,
    pub hidden: bool,
    pub shroud_object: u32,
    pub condition_bits: u128,
    pub opacity: f32,
    pub scale: f32,
}

pub enum NativeAuxiliaryMutation<'a> {
    Creating {
        receipt: &'a NativeAuxiliaryReceipt,
        initial_transform: Matrix4,
        sync_ms: u32,
    },
    TransformInstalled {
        receipt: &'a NativeAuxiliaryReceipt,
        transform: Matrix4,
    },
    ConditionsMarked {
        receipt: &'a NativeAuxiliaryReceipt,
        bits: u128,
    },
    Destroyed {
        receipt: &'a NativeAuxiliaryReceipt,
    },
}

impl NativeDrawableOwner<'_> {
    pub fn native_draw_candidates(&self) -> GameClientResult<Vec<NativeDrawCandidate>> {
        let mut result = Vec::new();
        for id in &self.drawables.native_drawable_order {
            let draw = self
                .drawables
                .drawable_map
                .get(id)
                .ok_or(GameClientError::DrawableNotFound(*id))?;
            if let Some(binding) = self.drawables.presentation_direct_drawable_bindings.get(id) {
                result.push(NativeDrawCandidate::Actor(binding.binding_key));
                continue;
            }
            let Some(basic) = draw.as_any().downcast_ref::<BasicDrawable>() else {
                continue;
            };
            let Some(generation) = basic.native_auxiliary_generation() else {
                continue;
            };
            if basic.get_object_id().is_some() {
                return Err(GameClientError::InvalidOperation(
                    "auxiliary Drawable unexpectedly Object-bound".into(),
                ));
            }
            result.push(NativeDrawCandidate::Aux(NativeAuxiliaryReceipt {
                id: *id,
                generation,
                owner: Arc::downgrade(self.auxiliary_identity),
            }));
        }
        Ok(result)
    }

    pub fn observe_auxiliary_drawable(
        &self,
        receipt: &NativeAuxiliaryReceipt,
    ) -> GameClientResult<Option<NativeAuxiliaryDrawObservation>> {
        let Some(draw) = self.drawables.drawable_map.get(&receipt.id) else {
            return Ok(None);
        };
        let basic = draw
            .as_any()
            .downcast_ref::<BasicDrawable>()
            .ok_or_else(|| {
                GameClientError::InvalidOperation("auxiliary receiver type replaced".into())
            })?;
        if !self.auxiliary_is_live(receipt) {
            return Ok(None);
        }
        Ok(Some(NativeAuxiliaryDrawObservation {
            base: basic.native_base_transform(),
            instance: basic.get_instance_transform(),
            hidden: basic.is_scene_effectively_hidden(),
            shroud_object: basic.shroud_status_object_id(),
            opacity: basic.get_opacity(),
            scale: basic.get_instance_scale(),
            condition_bits: {
                let flags = basic.get_model_condition_flags();
                (0..128.min(flags.get_bit_names().len()))
                    .fold(0u128, |bits, i| bits | ((flags.test(i) as u128) << i))
            },
        }))
    }

    pub fn auxiliary_is_live(&self, receipt: &NativeAuxiliaryReceipt) -> bool {
        self.drawables
            .drawable_map
            .get(&receipt.id)
            .filter(|draw| draw.get_object_id().is_none())
            .and_then(|draw| draw.as_any().downcast_ref::<BasicDrawable>())
            .is_some_and(|draw| {
                draw.native_auxiliary_generation() == Some(receipt.generation)
                    && Weak::ptr_eq(&receipt.owner, &Arc::downgrade(self.auxiliary_identity))
            })
    }

    /// Same registered, unbound constructor kernel as an actor. The wrapper
    /// deliberately never binds an Object or inserts an Object inverse-map row.
    pub fn create_auxiliary_drawable(
        &mut self,
        plan: Arc<AdmittedReceivingTemplate>,
        content: &mut game_engine::common::thing::module_factory::ModuleFactory,
        scale: f32,
        policy: ReceivingDrawablePolicy,
        birth: NativeDrawableBirthContext,
        logic_frame: u32,
        sync_ms: u32,
        observe: impl FnMut(NativeAuxiliaryMutation<'_>) -> GameClientResult<()>,
    ) -> GameClientResult<NativeAuxiliaryReceipt> {
        self.create_auxiliary_drawable_with_locality(
            plan, content, scale, policy, birth, logic_frame, sync_ms, None, observe,
        )
    }

    pub fn create_auxiliary_drawable_with_locality(
        &mut self,
        plan: Arc<AdmittedReceivingTemplate>,
        content: &mut game_engine::common::thing::module_factory::ModuleFactory,
        scale: f32,
        policy: ReceivingDrawablePolicy,
        birth: NativeDrawableBirthContext,
        logic_frame: u32,
        sync_ms: u32,
        locality: Option<&game_engine::common::audio::AudioSubmissionLocality>,
        mut observe: impl FnMut(NativeAuxiliaryMutation<'_>) -> GameClientResult<()>,
    ) -> GameClientResult<NativeAuxiliaryReceipt> {
        self.particles
            .admit_effective_definitions()
            .map_err(|error| GameClientError::SubsystemError(error.to_string()))?;
        let tod = policy.time_of_day.ok_or_else(|| {
            GameClientError::InvalidOperation(
                "authored objectless Drawable requires receiving time of day".into(),
            )
        })?;
        if content.receiving_name_keys_mut().is_none() {
            return Err(GameClientError::InvalidOperation(
                "objectless Drawable requires receiving name keys".into(),
            ));
        }
        let id = self.drawables.alloc_drawable_id();
        let mut draw = BasicDrawable::new(id);
        draw.admit_receiving_time_of_day(Some(tod));
        draw.set_instance_scale(scale);
        draw.initialize_native_constructor_template(
            Some(plan.name().to_owned()),
            Some(Arc::clone(&plan)),
        );
        let receipt = NativeAuxiliaryReceipt {
            id,
            generation: self.drawables.alloc_binding_generation(),
            owner: Arc::downgrade(self.auxiliary_identity),
        };
        draw.mark_native_auxiliary(receipt.generation);
        self.drawables.admit_native_drawable(id, Box::new(draw));
        let outcome =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> GameClientResult<()> {
                let draw = self
                    .drawables
                    .drawable_map
                    .get_mut(&id)
                    .unwrap()
                    .as_any_mut()
                    .downcast_mut::<BasicDrawable>()
                    .unwrap();
                observe(NativeAuxiliaryMutation::Creating {
                    receipt: &receipt,
                    initial_transform: draw.native_base_transform(),
                    sync_ms,
                })?;
                GameClient::initialize_receiving_drawable_modules_unbound(
                    draw,
                    &plan,
                    content.receiving_name_keys_mut().unwrap(),
                    policy,
                    logic_frame,
                )?;
                if let Some(audio_device) = self.audio_device {
                    let locality = locality.ok_or_else(|| {
                        GameClientError::InvalidOperation(
                            "bound receiving audio requires auxiliary Drawable locality".into(),
                        )
                    })?;
                    let mut audio = audio_device.lock().map_err(|_| {
                        GameClientError::SubsystemError(
                            "receiving audio manager lock poisoned during auxiliary Drawable construction".into(),
                        )
                    })?;
                    draw.finish_native_constructor_ambient_with_audio(
                        birth.loading_map, birth.in_load_game, tod, &mut audio, locality,
                    ).map_err(GameClientError::InvalidOperation)?;
                } else {
                    draw.finish_native_constructor_ambient_at(
                        birth.loading_map, birth.in_load_game, tod,
                    );
                }
                let flags = draw.get_model_condition_flags();
                let bits = (0..128.min(flags.get_bit_names().len()))
                    .fold(0u128, |bits, i| bits | ((flags.test(i) as u128) << i));
                observe(NativeAuxiliaryMutation::ConditionsMarked {
                    receipt: &receipt,
                    bits,
                })?;
                Ok(())
            }));
        let result = match outcome {
            Ok(result) => result,
            Err(payload) => {
                if let Err(error) = self
                    .drawables
                    .try_remove_for_map_attempt_with_audio(
                        id,
                        Some(&mut *self.particles),
                        self.audio_device,
                    )
                {
                    log::error!("auxiliary constructor panic cleanup failed: {error}");
                }
                match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    observe(NativeAuxiliaryMutation::Destroyed { receipt: &receipt })
                })) {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) => {
                        log::error!("auxiliary renderer panic cleanup failed: {error}")
                    }
                    Err(_) => log::error!("auxiliary renderer cleanup panicked"),
                }
                std::panic::resume_unwind(payload)
            }
        };
        if let Err(error) = result {
            let cleanup = self
                .drawables
                .try_remove_for_map_attempt_with_audio(
                    id,
                    Some(&mut *self.particles),
                    self.audio_device,
                );
            let renderer = observe(NativeAuxiliaryMutation::Destroyed { receipt: &receipt });
            return Err(GameClientError::InvalidOperation(format!(
                "objectless constructor: {error}; cleanup={cleanup:?}; renderer={renderer:?}"
            )));
        }
        Ok(receipt)
    }

    pub fn position_auxiliary_drawable(
        &mut self,
        receipt: &NativeAuxiliaryReceipt,
        position: [f32; 3],
        orientation: Option<f32>,
        mut observe: impl FnMut(NativeAuxiliaryMutation<'_>) -> GameClientResult<()>,
    ) -> GameClientResult<()> {
        if !self.auxiliary_is_live(receipt) {
            return Err(GameClientError::InvalidOperation(
                "stale or foreign objectless Drawable receipt".into(),
            ));
        }
        let draw = self
            .drawables
            .drawable_map
            .get_mut(&receipt.id)
            .unwrap()
            .as_any_mut()
            .downcast_mut::<BasicDrawable>()
            .unwrap();
        draw.set_position(Vector3::new(position[0], position[1], position[2]));
        observe(NativeAuxiliaryMutation::TransformInstalled {
            receipt,
            transform: draw.native_base_transform(),
        })?;
        if let Some(orientation) = orientation {
            // Source Rotate_Z orientation after the source Y/Z coordinate swap.
            draw.set_base_transform(Matrix4::rotation_y(-orientation));
            observe(NativeAuxiliaryMutation::TransformInstalled {
                receipt,
                transform: draw.native_base_transform(),
            })?;
        }
        Ok(())
    }
    pub fn set_auxiliary_scene_flags(
        &mut self,
        receipt: &NativeAuxiliaryReceipt,
        hidden: bool,
        shroud_object: Option<u32>,
    ) -> GameClientResult<()> {
        if !self.auxiliary_is_live(receipt) {
            return Err(GameClientError::InvalidOperation(
                "stale or foreign auxiliary scene flags".into(),
            ));
        }
        let draw = self
            .drawables
            .drawable_map
            .get_mut(&receipt.id)
            .unwrap()
            .as_any_mut()
            .downcast_mut::<BasicDrawable>()
            .unwrap();
        draw.set_drawable_hidden(hidden);
        if let Some(controller) = shroud_object {
            draw.set_shroud_status_object_id(controller);
        }
        Ok(())
    }

    pub fn retire_auxiliary_drawable(
        &mut self,
        receipt: &NativeAuxiliaryReceipt,
        mut observe: impl FnMut(NativeAuxiliaryMutation<'_>) -> GameClientResult<()>,
    ) -> GameClientResult<()> {
        if !self.auxiliary_is_live(receipt) {
            return Err(GameClientError::InvalidOperation(
                "stale or foreign auxiliary retirement".into(),
            ));
        }
        self.drawables
            .try_remove_for_map_attempt_with_audio(
                receipt.id,
                Some(&mut *self.particles),
                self.audio_device,
            )?;
        observe(NativeAuxiliaryMutation::Destroyed { receipt })
    }
}
