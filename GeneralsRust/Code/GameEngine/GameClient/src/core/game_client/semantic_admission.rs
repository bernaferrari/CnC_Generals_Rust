// One real CPU Drawable constructor shared by factory admission and presentation.
// Included in game_client so the existing owners and module kernels stay private.
pub struct NativeDrawableOwner<'a> {
    drawables: &'a mut drawable_world::DrawableWorld,
}
impl NativeDrawableWorld {
    pub fn borrow_native(&mut self) -> NativeDrawableOwner<'_> {
        NativeDrawableOwner { drawables: self }
    }
}
impl NativeDrawableOwner<'_> {
    pub fn binding_key(
        &self,
        epoch: u64,
        object: u32,
    ) -> Option<PresentationDirectDrawableBindingKey> {
        self.drawables
            .presentation_direct_drawable_state(epoch, object)
            .map(|state| state.binding_key)
    }
    /// One factory birth, independent of GPU/presentation residency. Never prunes
    /// another actor. The caller supplies the original pre-placement pose.
    pub fn admit_semantic_drawable_with_content(
        &mut self,
        entry: PresentationDrawableSync,
        sync_ms: u32,
        content: &mut game_engine::common::thing::module_factory::ModuleFactory,
        policy: ReceivingDrawablePolicy,
        mut observe: impl FnMut(PresentationDrawableMutation<'_>),
    ) -> GameClientResult<PresentationDirectDrawableBindingKey> {
        if entry.module_admission.authored_template().is_none() {
            return Err(GameClientError::InvalidOperation(
                "semantic birth requires its admitted receiving template".into(),
            ));
        }
        self.admit_fresh_drawable(
            &entry,
            sync_ms,
            Some(content),
            Some(policy),
            true,
            None,
            &mut observe,
        )
    }

    fn admit_fresh_drawable(
        &mut self,
        e: &PresentationDrawableSync,
        sync_ms: u32,
        mut content: Option<&mut game_engine::common::thing::module_factory::ModuleFactory>,
        policy: Option<ReceivingDrawablePolicy>,
        semantic_lifetime: bool,
        inherited_material_opacity: Option<f32>,
        observe: &mut impl FnMut(PresentationDrawableMutation<'_>),
    ) -> GameClientResult<PresentationDirectDrawableBindingKey> {
        if self
            .drawables
            .drawable_object_map
            .contains_key(&e.object_id)
        {
            return Err(GameClientError::InvalidOperation(
                "fresh Drawable birth already has an Object association".into(),
            ));
        }
        if e.receiving_science_player.is_none()
            && GameClient::fresh_drawable_requires_science(e, policy)
        {
            return Err(GameClientError::InvalidOperation(
                "authored science Drawable requires its receiving local-player observation".into(),
            ));
        }
        let visual_template_name = GameClient::presentation_visual_template_name(e).to_string();
        let id = self.drawables.alloc_drawable_id();
        let mut drawable = BasicDrawable::new(id);
        if let Some(opacity) = inherited_material_opacity {
            drawable.set_second_material_pass_opacity(opacity);
        }
        if !visual_template_name.is_empty() {
            drawable.set_template_name_with_receiving(
                Some(visual_template_name.clone()),
                e.module_admission.authored_template().cloned(),
            );
        }
        // Drawable.cpp338-344: registration precedes all module constructors.
        // Object binding is absent throughout construction and onObjectCreated.
        self.drawables.admit_native_drawable(id, Box::new(drawable));
        let result = (|| {
            let drawable = self
                .drawables
                .drawable_map
                .get_mut(&id)
                .unwrap()
                .as_any_mut()
                .downcast_mut::<BasicDrawable>()
                .unwrap();
            observe(PresentationDrawableMutation::Creating {
                source: e,
                initial_transform: drawable.native_base_transform(),
                sync_ms,
            });
            if let Some(admitted) = e.module_admission.authored_template() {
                let keys = content
                    .as_deref_mut()
                    .and_then(|owner| owner.receiving_name_keys_mut())
                    .ok_or_else(|| {
                        GameClientError::InvalidOperation(
                            "authored Drawable requires its receiving name-key owner".into(),
                        )
                    })?;
                let policy = policy.ok_or_else(|| {
                    GameClientError::InvalidOperation(
                        "authored Drawable requires receiving LOD policy".into(),
                    )
                })?;
                let modules = GameClient::create_receiving_modules_from_plan(
                    admitted,
                    keys,
                    policy,
                    (INVALID_ID, e.draw_logic_frame),
                )?;
                // Inter-module resolution sees every installed module, as CPP.
                for module in modules {
                    drawable.add_draw_module(module);
                }
                for slot in drawable.get_draw_modules_mut() {
                    if let Some(module) = slot.logic_module_mut() {
                        gamelogic::object::drawable::create_native_draw_module_unbound(
                            module,
                            admitted.per_unit_sound_event("TruckLandingSound"),
                            admitted.per_unit_sound_event("TruckPowerslideSound"),
                        );
                    }
                }
            } else {
                GameClient::attach_presentation_specialized_draw_modules(drawable, e);
            }
            // GameLogic::sendObjectCreated binds only after newDrawable returns.
            drawable.set_object_id(Some(e.object_id));
            for slot in drawable.get_draw_modules_mut() {
                if let Some(module) = slot.logic_module_mut() {
                    gamelogic::object::drawable::bind_native_draw_module_owner(
                        module,
                        e.object_id,
                        e.draw_indicator_color,
                    );
                    if let Some(beacon) = module
                        .as_any_mut()
                        .downcast_mut::<BeaconClientUpdateModule>()
                    {
                        beacon.bind_owner_id(e.object_id);
                    } else if let Some(sway) =
                        module.as_any_mut().downcast_mut::<SwayClientUpdateModule>()
                    {
                        sway.bind_owner_id(e.object_id);
                    } else if let Some(animated) = module
                        .as_any_mut()
                        .downcast_mut::<AnimatedParticleSysBoneClientUpdateModule>(
                    ) {
                        animated.bind_owner_id(e.object_id);
                    }
                }
            }
            drawable.set_position(Vector3::new(e.position[0], e.position[1], e.position[2]));
            observe(PresentationDrawableMutation::TransformInstalled {
                source: e,
                transform: drawable.native_base_transform(),
            });
            drawable.set_instance_transform(
                Matrix4::rotation_y(e.orientation)
                    .mul(&Matrix4::rotation_z(e.float_yaw))
                    .mul(&Matrix4::rotation_x(e.float_pitch)),
            );
            observe(PresentationDrawableMutation::TransformInstalled {
                source: e,
                transform: drawable.native_base_transform(),
            });
            GameClient::stamp_presentation_object_residual(drawable, e);
            let flags = drawable.get_model_condition_flags();
            let bits = (0..128.min(flags.get_bit_names().len()))
                .fold(0u128, |bits, i| bits | ((flags.test(i) as u128) << i));
            observe(PresentationDrawableMutation::ConditionsMarked { source: e, bits });
            let native_laser = GameClient::owned_laser_count(drawable) == 1;
            GameClient::bind_owned_laser_module(drawable, id);
            if native_laser {
                self.drawables.owned_laser_order.insert(0, id);
            }
            self.drawables.drawable_object_map.insert(e.object_id, id);
            let binding_key = PresentationDirectDrawableBindingKey {
                host_epoch: e.host_epoch,
                object_id: e.object_id,
                drawable_id: id,
                binding_generation: self.drawables.alloc_binding_generation(),
            };
            self.drawables.presentation_direct_drawable_bindings.insert(
                id,
                PresentationDirectDrawableBinding {
                    semantic_lifetime,
                    binding_key,
                    visual_template_name,
                },
            );
            Ok(binding_key)
        })();
        if result.is_err() {
            // The failed receiver was never bound/admitted to another Object.
            self.drawables
                .native_drawable_order
                .retain(|known| *known != id);
            self.drawables.drawable_map.remove(&id);
            observe(PresentationDrawableMutation::Destroyed {
                host_epoch: e.host_epoch,
                object_id: e.object_id,
            });
        }
        result
    }
}

#[cfg(test)]
mod semantic_admission_tests {
    use super::*;
    use game_engine::common::ini::INILoadType;
    use game_engine::common::name_key_generator::NameKeyNamespace;
    use game_engine::common::thing::module::StaticGameLodLevel;
    use game_engine::common::thing::module_factory::ModuleFactory;
    use game_engine::common::thing::thing_template::{ObjectIniFields, ReceivingTemplateKind};

    fn content(
        rate: f32,
    ) -> (
        ModuleFactory,
        Arc<game_engine::common::thing::thing_template::AdmittedReceivingTemplate>,
    ) {
        let mut factory = ModuleFactory::new_receiving(NameKeyNamespace::new());
        gamelogic::contain_module_overrides::register_native_module_overrides(&mut factory)
            .unwrap();
        let source = format!("Draw = W3DTankDraw TankTag\nTreadAnimationRate = {rate}\nEnd\nEnd");
        let lines = source.lines().collect::<Vec<_>>();
        let fields = ObjectIniFields::capture_body(&lines, 0).0;
        let plan = ThingTemplate::new()
            .admit_named_receiving_module_source(
                "SemanticTank",
                &fields,
                INILoadType::Overwrite,
                ReceivingTemplateKind::Object,
                &mut factory,
            )
            .unwrap();
        (factory, Arc::new(plan))
    }
    fn policy() -> ReceivingDrawablePolicy {
        ReceivingDrawablePolicy {
            use_draw_module_lod: true,
            static_game_lod: StaticGameLodLevel::High,
            use_tree_sway: true,
        }
    }
    fn entry(
        id: u32,
        plan: Arc<game_engine::common::thing::thing_template::AdmittedReceivingTemplate>,
    ) -> PresentationDrawableSync {
        let mut entry = super::tests::presentation_drawable_sync_for_test(
            id,
            5,
            "SemanticTank",
            false,
            false,
            [0.0; 3],
            0.0,
        );
        entry.module_admission = PresentationModuleAdmission::Authored(plan);
        entry
    }
    fn rate(client: &mut GameClient, id: DrawableId) -> f32 {
        client
            .drawables
            .drawable_map
            .get_mut(&id)
            .unwrap()
            .as_any_mut()
            .downcast_mut::<BasicDrawable>()
            .unwrap()
            .get_draw_modules_mut()[0]
            .logic_module_mut()
            .unwrap()
            .get_module_data()
            .as_any()
            .downcast_ref::<W3DTankDrawModuleData>()
            .unwrap()
            .tread_animation_rate
    }
    #[test]
    fn semantic_cpu_birth_preserves_same_id_peers_order_and_nonresident_lifetime() {
        let (mut catalog, plan) = content(2.0);
        let (mut peer_catalog, peer_plan) = content(7.0);
        let mut client = GameClient::new().unwrap();
        let mut peer = GameClient::new().unwrap();
        let child = client
            .split_frame_snow_and_drawable_owner()
            .2
            .admit_semantic_drawable_with_content(
                entry(77, Arc::clone(&plan)),
                0,
                &mut catalog,
                policy(),
                |_| {},
            )
            .unwrap();
        let parent = client
            .split_frame_snow_and_drawable_owner()
            .2
            .admit_semantic_drawable_with_content(
                entry(42, Arc::clone(&plan)),
                0,
                &mut catalog,
                policy(),
                |_| {},
            )
            .unwrap();
        let peer_parent = peer
            .split_frame_snow_and_drawable_owner()
            .2
            .admit_semantic_drawable_with_content(
                entry(42, peer_plan),
                0,
                &mut peer_catalog,
                policy(),
                |_| {},
            )
            .unwrap();
        assert_ne!(parent.drawable_id.0, parent.object_id);
        assert!((rate(&mut client, parent.drawable_id) - 2.0 / 30.0).abs() < 0.00001);
        assert!((rate(&mut peer, peer_parent.drawable_id) - 7.0 / 30.0).abs() < 0.00001);
        assert_eq!(
            client.drawables.native_drawable_order,
            vec![parent.drawable_id, child.drawable_id]
        );
        assert_eq!(
            peer.drawables.native_drawable_order,
            vec![peer_parent.drawable_id]
        );
        assert_eq!(client.get_drawable_for_object(77), Some(child.drawable_id));
        assert_eq!(client.get_drawable_for_object(42), Some(parent.drawable_id));
        assert_eq!(
            client
                .sync_presentation_drawables(std::iter::empty())
                .unwrap(),
            (0, 0, 0)
        );
        assert_eq!(client.get_drawable_for_object(42), Some(parent.drawable_id));
        assert_eq!(
            client
                .sync_presentation_drawables_observed_with_content(
                    [entry(42, plan)],
                    0,
                    &mut catalog,
                    policy(),
                    |_| {},
                )
                .unwrap(),
            (0, 1, 0)
        );
        assert_eq!(client.get_drawable_for_object(77), Some(child.drawable_id));
        assert_eq!(
            peer.get_drawable_for_object(42),
            Some(peer_parent.drawable_id)
        );
    }
    #[test]
    fn detached_cpu_owner_admits_without_client_or_gpu_and_rejects_duplicate_binding() {
        let (mut catalog, plan) = content(2.0);
        let mut world = NativeDrawableWorld::new();
        let key = world
            .borrow_native()
            .admit_semantic_drawable_with_content(
                entry(42, Arc::clone(&plan)),
                0,
                &mut catalog,
                policy(),
                |_| {},
            )
            .unwrap();
        assert_eq!(world.borrow_native().binding_key(5, 42), Some(key));
        assert!(
            world
                .borrow_native()
                .admit_semantic_drawable_with_content(
                    entry(42, plan),
                    0,
                    &mut catalog,
                    policy(),
                    |_| {},
                )
                .is_err()
        );
        assert_eq!(world.native_drawable_order, vec![key.drawable_id]);
        assert_eq!(world.drawable_map.len(), 1);
    }
    #[test]
    fn failed_constructor_removes_registered_receiver_without_pruning_peer() {
        let (mut catalog, plan) = content(2.0);
        let mut world = NativeDrawableWorld::new();
        let peer = world
            .borrow_native()
            .admit_semantic_drawable_with_content(
                entry(77, Arc::clone(&plan)),
                0,
                &mut catalog,
                policy(),
                |_| {},
            )
            .unwrap();
        let mut wrong_content = ModuleFactory::new();
        let mut phases = Vec::new();
        assert!(
            world
                .borrow_native()
                .admit_semantic_drawable_with_content(
                    entry(42, plan),
                    0,
                    &mut wrong_content,
                    policy(),
                    |event| match event {
                        PresentationDrawableMutation::Creating { .. } =>
                            phases.push("registered-before-construction"),
                        PresentationDrawableMutation::Destroyed { .. } =>
                            phases.push("retired-failed-receiver"),
                        _ => {}
                    },
                )
                .is_err()
        );
        assert_eq!(
            phases,
            vec!["registered-before-construction", "retired-failed-receiver"]
        );
        assert_eq!(world.native_drawable_order, vec![peer.drawable_id]);
        assert_eq!(world.drawable_map.len(), 1);
        assert_eq!(world.borrow_native().binding_key(5, 42), None);
    }
}

impl GameClient {
    fn fresh_drawable_requires_science(
        e: &PresentationDrawableSync,
        policy: Option<ReceivingDrawablePolicy>,
    ) -> bool {
        e.module_admission
            .authored_template()
            .is_some_and(|template| {
                template.draw_modules().iter().any(|entry| {
                    !policy.is_some_and(|policy| {
                        policy.use_draw_module_lod
                            && (entry.data.get_minimum_required_game_lod() as i32)
                                > (policy.static_game_lod as i32)
                    }) && entry
                        .data
                        .as_any()
                        .downcast_ref::<W3DScienceModelDrawModuleData>()
                        .is_some_and(|data| {
                            data.required_science != game_engine::common::rts::SCIENCE_INVALID
                        })
                })
            })
    }
}
