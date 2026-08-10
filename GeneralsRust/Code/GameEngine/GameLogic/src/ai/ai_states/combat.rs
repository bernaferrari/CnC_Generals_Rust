/// AI Attack State
#[derive(Debug)]
pub struct AIAttackState {
    follow: bool,
    attacking_object: bool,
    force_attacking: bool,
    attack_area: bool,
    original_victim_pos: Coord3D,
    victim_team: Option<u32>,
}

impl AIAttackState {
    pub fn new(
        follow: bool,
        attacking_object: bool,
        force_attacking: bool,
        attack_area: bool,
    ) -> Self {
        Self {
            follow,
            attacking_object,
            force_attacking,
            attack_area,
            original_victim_pos: Coord3D::new(0.0, 0.0, 0.0),
            victim_team: None,
        }
    }

    fn choose_weapon(&self, context: &AIStateMachineContext) -> bool {
        let Some(owner_arc) = OBJECT_REGISTRY.get_object(context.owner_id) else {
            return false;
        };

        let mut owner = match owner_arc.write() {
            Ok(guard) => guard,
            Err(_) => return false,
        };

        let cmd_source = owner
            .get_ai()
            .map(|ai| ai.get_last_command_source())
            .unwrap_or(CommandSourceType::FromAi);

        let found = if self.attacking_object {
            let Some(target_id) = context.goal_object else {
                return false;
            };
            let Some(target_arc) = OBJECT_REGISTRY.get_object(target_id) else {
                return false;
            };
            let Ok(target) = target_arc.read() else {
                return false;
            };
            owner.choose_best_weapon_for_target(
                &target,
                WeaponChoiceCriteria::PreferMostDamage,
                cmd_source,
            )
        } else {
            owner.choose_best_weapon_for_target_id(
                INVALID_ID,
                WeaponChoiceCriteria::PreferMostDamage,
                cmd_source,
            )
        };

        owner.adjust_model_condition_for_weapon_status();
        found
    }
}

impl AIState for AIAttackState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        // Wave 254: empty dual-world → fail-closed (no factory owner).
        if dual_world_registry_unavailable() {
            return StateReturnType::Failed;
        }

        let Some(owner_arc) = OBJECT_REGISTRY.get_object(context.owner_id) else {
            return StateReturnType::Failed;
        };
        {
            let Ok(owner) = owner_arc.read() else {
                return StateReturnType::Failed;
            };
            if owner.test_status(ObjectStatusTypes::UnderConstruction) {
                return StateReturnType::Failed;
            }
            if owner.is_out_of_ammo() && !owner.is_kind_of(KindOf::Projectile) {
                return StateReturnType::Failed;
            }
        }

        if self.attacking_object {
            let Some(target_id) = context.goal_object else {
                return StateReturnType::Failed;
            };
            let Some(target_arc) = OBJECT_REGISTRY.get_object(target_id) else {
                return StateReturnType::Failed;
            };
            let Ok(target) = target_arc.read() else {
                return StateReturnType::Failed;
            };
            if target.is_effectively_dead() {
                return StateReturnType::Failed;
            }
            self.original_victim_pos = *target.get_position();
            self.victim_team = target.get_team_id();
        } else {
            let Some(pos) = context.goal_position else {
                return StateReturnType::Failed;
            };
            self.original_victim_pos = pos;
        }

        if !self.choose_weapon(context) {
            return StateReturnType::Failed;
        }

        if let Ok(mut owner) = owner_arc.write() {
            if let Some((weapon, _slot)) = owner.get_current_weapon() {
                if weapon.get_lock_on_range() > 0.0 {
                    owner.set_status(
                        ObjectStatusMaskType::from(ObjectStatusTypes::IgnoringStealth),
                        true,
                    );
                }
            }
            owner.set_status(
                ObjectStatusMaskType::from(ObjectStatusTypes::IsAttacking),
                true,
            );
        }

        StateReturnType::Continue
    }

    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        // Wave 254: empty dual-world → fail-closed (no factory owner).
        if dual_world_registry_unavailable() {
            return StateReturnType::Failed;
        }

        // Attack state update logic
        let Some(owner_arc) = OBJECT_REGISTRY.get_object(context.owner_id) else {
            return StateReturnType::Failed;
        };
        let Ok(owner) = owner_arc.read() else {
            return StateReturnType::Failed;
        };
        if owner.is_out_of_ammo() && !owner.is_kind_of(KindOf::Projectile) {
            return StateReturnType::Failed;
        }

        if self.attacking_object {
            let Some(target_id) = context.goal_object else {
                return StateReturnType::Complete;
            };

            let Some(target_arc) = OBJECT_REGISTRY.get_object(target_id) else {
                return StateReturnType::Complete;
            };
            let Ok(target) = target_arc.read() else {
                return StateReturnType::Failed;
            };
            if target.is_effectively_dead() {
                return StateReturnType::Complete;
            }

            let relationship = owner.relationship_to(&target);
            if !target.test_status(ObjectStatusTypes::CanAttack) {
                if let Some(contain) = target.get_contain() {
                    if let Ok(contain_guard) = contain.lock() {
                        if contain_guard.is_garrisonable()
                            && contain_guard.get_contained_count() == 0
                            && relationship == Relationship::Neutral
                        {
                            return StateReturnType::Failed;
                        }
                    }
                }
            }

            if relationship != Relationship::Enemies {
                return StateReturnType::Failed;
            }

            if out_of_weapon_range_object(context) {
                return StateReturnType::Failed;
            }

            if want_to_squish_target(context) {
                return StateReturnType::Failed;
            }
        } else {
            if context.goal_position.is_none() {
                return StateReturnType::Failed;
            }

            if out_of_weapon_range_position(context) {
                return StateReturnType::Failed;
            }
        }

        if !self.choose_weapon(context) {
            return StateReturnType::Failed;
        }
        let Some((weapon, _slot)) = owner.get_current_weapon() else {
            return StateReturnType::Failed;
        };
        if weapon.get_max_shot_count() <= 0 {
            return StateReturnType::Failed;
        }

        StateReturnType::Continue
    }

    fn on_exit(&mut self, context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        // Wave 254: empty dual-world → no factory owner/target.
        if dual_world_registry_unavailable() {
            return;
        }

        let _ = OBJECT_REGISTRY.with_object_mut(context.owner_id, |owner| {
            owner.set_status(
                ObjectStatusMaskType::from(ObjectStatusTypes::IsAttacking),
                false,
            );
            owner.set_status(
                ObjectStatusMaskType::from(ObjectStatusTypes::IgnoringStealth),
                false,
            );
        });
    }

    fn get_state_type(&self) -> AIStateType {
        if self.attacking_object {
            if self.force_attacking {
                AIStateType::ForceAttackObject
            } else if self.follow {
                AIStateType::AttackAndFollowObject
            } else {
                AIStateType::AttackObject
            }
        } else {
            if self.attack_area {
                AIStateType::AttackArea
            } else if self.follow {
                AIStateType::AttackMoveTo
            } else {
                AIStateType::AttackPosition
            }
        }
    }

    fn is_attack(&self) -> bool {
        true
    }
}

/// AI Guard State
#[derive(Debug)]
pub struct AIGuardState {
    guard_position: Option<Coord3D>,
    guard_object: Option<ObjectID>,
    guard_mode: GuardMode,
    scan_timer: u32,
    last_enemy_scan_time: u32,
    guard_machine: Option<AIGuardMachine>,
}

impl AIGuardState {
    pub fn new() -> Self {
        Self {
            guard_position: None,
            guard_object: None,
            guard_mode: GuardMode::Normal,
            scan_timer: 0,
            last_enemy_scan_time: 0,
            guard_machine: None,
        }
    }
}

impl AIState for AIGuardState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        // Wave 254: empty dual-world → fail-closed (no factory owner).
        if dual_world_registry_unavailable() {
            return StateReturnType::Failed;
        }

        self.guard_position = context.goal_position;
        self.guard_object = context.goal_object;
        self.guard_mode = match context.int_value {
            0 => GuardMode::Normal,
            1 => GuardMode::GuardWithoutPursuit,
            2 => GuardMode::GuardFlyingUnitsOnly,
            _ => GuardMode::Normal,
        };

        if let Some(owner_arc) = get_legacy_object(context.owner_id) {
            let mut guard_machine = AIGuardMachine::new(Arc::downgrade(&owner_arc));

            if let Some(target_id) = context.goal_object {
                if let Some(target_arc) = get_legacy_object(target_id) {
                    guard_machine.set_target_to_guard(Some(&target_arc));
                }
            } else if let Some(pos) = context.goal_position {
                guard_machine.set_target_position_to_guard(&pos);
            } else if let Ok(owner_guard) = owner_arc.read() {
                guard_machine.set_target_position_to_guard(owner_guard.get_position());
            }

            guard_machine.set_guard_mode(self.guard_mode);
            if guard_machine.init_default_state().is_failure() {
                return StateReturnType::Failed;
            }
            let result = guard_machine.set_state(GuardStateType::Return);
            self.guard_machine = Some(guard_machine);
            return result;
        }

        StateReturnType::Continue
    }

    fn update(&mut self, _context: &mut AIStateMachineContext) -> StateReturnType {
        if let Some(guard_machine) = self.guard_machine.as_mut() {
            return guard_machine.update();
        }

        // Guard behavior - scan for enemies, respond to threats
        self.scan_timer += 1;

        if self.scan_timer >= 30 {
            // Scan every second
            self.scan_timer = 0;
            self.last_enemy_scan_time += 30;

            // Scan for enemies in guard range
            // If enemy found, attack based on guard mode
            // Guard mode influences pursuit/target filters
            // Guard modes influence pursuit behavior when an enemy is found
        }

        StateReturnType::Continue
    }

    fn on_exit(&mut self, _context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        if let Some(mut guard_machine) = self.guard_machine.take() {
            let _ = guard_machine.halt();
        }
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::Guard
    }

    fn is_attack(&self) -> bool {
        self.guard_machine
            .as_ref()
            .map(|machine| machine.is_in_attack_state())
            .unwrap_or(false)
    }

    fn is_guard_idle(&self) -> bool {
        self.guard_machine
            .as_ref()
            .map(|machine| machine.is_in_guard_idle_state())
            .unwrap_or(true)
    }
}

