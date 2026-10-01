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
        OBJECT_REGISTRY
            .with_object_mut(context.owner_id, |owner| {
                let cmd_source = owner
                    .get_ai()
                    .map(|ai| ai.get_last_command_source())
                    .unwrap_or(CommandSourceType::FromAi);

                let found = if self.attacking_object {
                    let Some(target_id) = context.goal_object else {
                        return false;
                    };
                    let Some(hit) = OBJECT_REGISTRY.with_object(target_id, |target| {
                        owner.choose_best_weapon_for_target(
                            target,
                            WeaponChoiceCriteria::PreferMostDamage,
                            cmd_source,
                        )
                    }) else {
                        return false;
                    };
                    hit
                } else {
                    owner.choose_best_weapon_for_target_id(
                        INVALID_ID,
                        WeaponChoiceCriteria::PreferMostDamage,
                        cmd_source,
                    )
                };

                owner.adjust_model_condition_for_weapon_status();
                found
            })
            .unwrap_or(false)
    }
}

fn clear_team_target_if_victim(owner: &crate::object::Object, victim_id: ObjectID) {
    if let Some(team_arc) = owner.get_team() {
        if let Ok(mut team_guard) = team_arc.write() {
            crate::ai::states::clear_team_target_object_if_victim(&mut team_guard, victim_id);
        }
    }
}


impl AIState for AIAttackState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        // Wave 254: empty dual-world → fail-closed (no factory owner).
        if dual_world_registry_unavailable() {
            return StateReturnType::Failed;
        }

        let owner_ok = OBJECT_REGISTRY.with_object(context.owner_id, |owner| {
            if owner.test_status(ObjectStatusTypes::UnderConstruction) {
                return false;
            }
            !(owner.is_out_of_ammo() && !owner.is_kind_of(KindOf::Projectile))
        });
        if owner_ok != Some(true) {
            return StateReturnType::Failed;
        }

        if self.attacking_object {
            let Some(target_id) = context.goal_object else {
                return StateReturnType::Failed;
            };
            let victim = OBJECT_REGISTRY.with_object(target_id, |target| {
                if target.is_effectively_dead() {
                    return None;
                }
                Some((*target.get_position(), target.get_team_id()))
            });
            let Some(Some((pos, team))) = victim else {
                return StateReturnType::Failed;
            };
            self.original_victim_pos = pos;
            self.victim_team = team;
            // C++ AIAttackFireWeaponState::onEnter seeds AttackCommonTarget (AIStates.cpp:5153-5156).
            let team_arc = OBJECT_REGISTRY.with_object(context.owner_id, |owner| owner.get_team());
            if let Some(Some(team_arc)) = team_arc {
                if let Ok(mut team_guard) = team_arc.write() {
                    crate::ai::states::seed_team_target_if_attack_common(&mut team_guard, target_id);
                }
            }
        } else {
            let Some(pos) = context.goal_position else {
                return StateReturnType::Failed;
            };
            self.original_victim_pos = pos;
        }

        if !self.choose_weapon(context) {
            return StateReturnType::Failed;
        }

        let _ = OBJECT_REGISTRY.with_object_mut(context.owner_id, |owner| {
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
        });

        StateReturnType::Continue
    }

    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        // Wave 254: empty dual-world → fail-closed (no factory owner).
        if dual_world_registry_unavailable() {
            return StateReturnType::Failed;
        }

        let pre = OBJECT_REGISTRY.with_object(context.owner_id, |owner| {
            owner.is_out_of_ammo() && !owner.is_kind_of(KindOf::Projectile)
        });
        if pre != Some(false) {
            return StateReturnType::Failed;
        }

        if self.attacking_object {
            let Some(target_id) = context.goal_object else {
                return StateReturnType::Complete;
            };

            let verdict = OBJECT_REGISTRY.with_object(context.owner_id, |owner| {
                OBJECT_REGISTRY.with_object(target_id, |target| {
                    if target.is_effectively_dead() {
                        return StateReturnType::Complete;
                    }
                    let relationship = owner.relationship_to(target);
                    if !target.test_status(ObjectStatusTypes::CanAttack) {
                        if let Some(contain) = target.get_contain() {
                            if contain.is_garrisonable()
                                && contain.get_contained_count() == 0
                                && relationship == Relationship::Neutral
                            {
                                return StateReturnType::Failed;
                            }
                        }
                    }
                    if relationship != Relationship::Enemies {
                        return StateReturnType::Failed;
                    }
                    StateReturnType::Continue
                })
            });
            match verdict {
                None | Some(None) => return StateReturnType::Complete,
                Some(Some(StateReturnType::Failed)) => {
                    context.goal_object = None;
                    let _ = OBJECT_REGISTRY.with_object(context.owner_id, |owner| {
                        clear_team_target_if_victim(owner, target_id);
                    });
                    return StateReturnType::Failed;
                }
                Some(Some(StateReturnType::Complete)) => return StateReturnType::Complete,
                Some(Some(StateReturnType::Continue)) => {}
                Some(Some(other)) => return other,
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
        let shots_ok = OBJECT_REGISTRY.with_object(context.owner_id, |owner| {
            match owner.get_current_weapon() {
                Some((weapon, _slot)) => weapon.get_max_shot_count() > 0,
                None => false,
            }
        });
        if shots_ok != Some(true) {
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
            owner.clear_leech_range_mode_for_all_weapons();
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

        if let Some(owner_id) = get_legacy_object(context.owner_id) {
            let mut guard_machine = AIGuardMachine::new(owner_id);

            if let Some(target_id) = context.goal_object {
                if let Some(target_id) = get_legacy_object(target_id) {
                    guard_machine.set_target_to_guard(Some(target_id));
                }
            } else if let Some(pos) = context.goal_position {
                guard_machine.set_target_position_to_guard(&pos);
            } else if let Some(pos) = crate::object::registry::OBJECT_REGISTRY
                .with_object(owner_id, |owner_guard| *owner_guard.get_position())
            {
                guard_machine.set_target_position_to_guard(&pos);
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

