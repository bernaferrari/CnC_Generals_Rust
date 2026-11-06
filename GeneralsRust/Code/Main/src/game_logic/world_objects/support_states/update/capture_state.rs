use super::*;

impl GameLogic {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn tick_capturing_support_state(
        &mut self,
        object_id: ObjectId,
        team: Team,
        owner_player_id: Option<u32>,
        target_id: Option<ObjectId>,
        can_move: bool,
        is_alive: bool,
        dt: f32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let Some(capture_target_id) = target_id else {
            self.abort_capture_channel(object_id)?;
            return Ok(());
        };

        let (
            capture_power,
            capture_start_range,
            capture_unpack_time_ms,
            capture_preparation_time_ms,
            capture_pack_time_ms,
            capture_channel,
            capturer_moving,
        ) = self
            .objects
            .get(&object_id)
            .map(|obj| {
                (
                    obj.thing().template.capture_power,
                    obj.thing().template.capture_start_ability_range,
                    obj.thing().template.capture_unpack_time_ms,
                    obj.thing().template.capture_preparation_time_ms,
                    obj.thing().template.capture_pack_time_ms,
                    obj.capture_channel,
                    obj.host_ai_is_moving(),
                )
            })
            .unwrap_or((
                crate::game_logic::CapturePowerKind::None,
                None,
                None,
                None,
                None,
                None,
                false,
            ));
        let Some(power_type) = capture_power.special_power_type() else {
            self.abort_capture_channel(object_id)?;
            return Ok(());
        };

        // Once C++ `startPacking` has run, the target may already
        // have defected or vanished.  Packing is deliberately
        // independent of capture legality and completes before
        // this source becomes idle again.
        if let Some(channel) = capture_channel {
            if channel.phase == crate::game_logic::CaptureChannelPhase::Packing {
                let remaining = (channel.remaining_seconds - dt.max(0.0)).max(0.0);
                if remaining > CAPTURE_CHANNEL_COMPLETE_EPSILON {
                    if let Some(object) = self.objects.get_mut(&object_id) {
                        object.capture_channel = Some(crate::game_logic::CaptureChannelState {
                            phase: crate::game_logic::CaptureChannelPhase::Packing,
                            remaining_seconds: remaining,
                        });
                    }
                } else {
                    self.finish_capture_channel(object_id)?;
                }
                return Ok(());
            }
        }
        // C++ SpecialAbilityUpdate.cpp:209-220: isMoving && isPowerCurrentlyInUse
        // && !m_facingInitiated → onExit. Live capture has no facing phase, so any
        // movement during unpack/prep (flag raise) aborts. Approach (no channel)
        // stays legal.
        if capturer_moving
            && matches!(
                capture_channel.map(|c| c.phase),
                Some(
                    crate::game_logic::CaptureChannelPhase::Unpacking
                        | crate::game_logic::CaptureChannelPhase::Preparing,
                )
            )
        {
            self.abort_capture_channel(object_id)?;
            return Ok(());
        }

        let Some((target_position, target_team)) = self
            .objects
            .get(&capture_target_id)
            .map(|target| (target.get_position(), target.team))
        else {
            self.abort_capture_channel(object_id)?;
            return Ok(());
        };

        // All timing fields come from the same exact
        // SpecialAbilityUpdate module.  A partial/unsupported
        // parse must not invent a zero-duration capture ability.
        let Some(authored_range) = capture_start_range else {
            self.abort_capture_channel(object_id)?;
            return Ok(());
        };
        let Some(unpack_time_ms) = capture_unpack_time_ms else {
            self.abort_capture_channel(object_id)?;
            return Ok(());
        };
        let Some(preparation_time_ms) = capture_preparation_time_ms else {
            self.abort_capture_channel(object_id)?;
            return Ok(());
        };
        let Some(pack_time_ms) = capture_pack_time_ms else {
            self.abort_capture_channel(object_id)?;
            return Ok(());
        };

        // C++ checks the target at issue/start-preparation and
        // continues to abort if an ally/dead/immune/garrisoned
        // target is no longer legal.  Do not re-demand readiness:
        // the timer begins only below in start_capture_preparation.
        if !self.can_unit_capture_building(object_id, capture_target_id, false) {
            self.abort_capture_channel(object_id)?;
            return Ok(());
        }

        // C++ `isWithinStartAbilityRange`: 2D bounding-sphere vs
        // authored StartAbilityRange, then ApproachRequiresLOS.
        if capture_channel.is_none()
            && can_move
            && !self.leftover_sa_within_start_range(object_id, capture_target_id, authored_range)
        {
            if self.assign_unit_path_ignoring(
                object_id,
                target_position,
                &[],
                Some(capture_target_id),
            ) {
                if let Some(obj) = self.objects.get_mut(&object_id) {
                    {
                        obj.set_ai_state(
                            AIState::Capturing,
                            &mut crate::game_logic::ObjectAudio::queue(
                                &mut self.queued_audio_events,
                            ),
                        );
                        if !obj.movement.path.is_empty() {
                            obj.set_status_moving(true);
                        }
                    }
                }
            }
            return Ok(());
        }

        // C++ initializes `STATE_UNPACKED` immediately when
        // UnpackTime is zero; otherwise the first logical tick
        // after entering range consumes the unpack timer.
        let mut preparation_complete = false;
        match capture_channel {
            None => {
                if unpack_time_ms > 0 {
                    let factor = self
                        .objects
                        .get(&object_id)
                        .map(|object| object.thing().template.capture_pack_unpack_variation_factor)
                        .unwrap_or(0.0);
                    let unpack_time_ms = crate::game_logic::thing::vary_pack_unpack_duration_ms(
                        unpack_time_ms,
                        factor,
                        &self.logic_random,
                    );
                    if let Some(object) = self.objects.get_mut(&object_id) {
                        object.capture_channel = Some(crate::game_logic::CaptureChannelState::new(
                            crate::game_logic::CaptureChannelPhase::Unpacking,
                            unpack_time_ms,
                        ));
                    }
                    self.begin_capture_unpacking_pose(object_id, capture_power);

                    return Ok(());
                }
                if !self.start_capture_preparation_with_effects(
                    object_id,
                    capture_target_id,
                    capture_power,
                    preparation_time_ms,
                    effects,
                ) {
                    self.abort_capture_channel(object_id)?;
                    return Ok(());
                }
                preparation_complete = preparation_time_ms == 0;
            }
            Some(channel) if channel.phase == crate::game_logic::CaptureChannelPhase::Unpacking => {
                let remaining = (channel.remaining_seconds - dt.max(0.0)).max(0.0);
                if remaining > CAPTURE_CHANNEL_COMPLETE_EPSILON {
                    if let Some(object) = self.objects.get_mut(&object_id) {
                        object.capture_channel = Some(crate::game_logic::CaptureChannelState {
                            phase: crate::game_logic::CaptureChannelPhase::Unpacking,
                            remaining_seconds: remaining,
                        });
                    }
                    return Ok(());
                }
                if !self.start_capture_preparation_with_effects(
                    object_id,
                    capture_target_id,
                    capture_power,
                    preparation_time_ms,
                    effects,
                ) {
                    self.abort_capture_channel(object_id)?;
                    return Ok(());
                }
                preparation_complete = preparation_time_ms == 0;
            }
            Some(channel) if channel.phase == crate::game_logic::CaptureChannelPhase::Preparing => {
                let remaining = (channel.remaining_seconds - dt.max(0.0)).max(0.0);
                if remaining > CAPTURE_CHANNEL_COMPLETE_EPSILON {
                    // C++ `continuePreparation` restarts infantry
                    // capture ReloadTime every preparation frame.
                    // Black Lotus instead resets its zero timer at
                    // the successful trigger below.
                    if capture_power != crate::game_logic::CapturePowerKind::BlackLotus {
                        if let Some(object) = self.objects.get_mut(&object_id) {
                            object.start_power_recharge(&power_type);
                        }
                    }
                    if let Some(object) = self.objects.get_mut(&object_id) {
                        object.capture_channel = Some(crate::game_logic::CaptureChannelState {
                            phase: crate::game_logic::CaptureChannelPhase::Preparing,
                            remaining_seconds: remaining,
                        });
                    }
                    self.apply_leftover_capture_fx(
                        object_id,
                        capture_target_id,
                        remaining,
                        preparation_time_ms,
                    );
                    return Ok(());
                }
                preparation_complete = true;
            }
            Some(_) => unreachable!("Packing is handled before capture legality"),
        }

        if !preparation_complete {
            return Ok(());
        }

        // C++ checks an enemy trap again at trigger; one planted
        // during the visible preparation bar can still interrupt
        // the actual defection.
        let target_is_trapped = self
            .objects
            .get(&capture_target_id)
            .is_some_and(|target| target.status.booby_trapped);
        if target_is_trapped {
            let _ = self.detonate_booby_trap_at_with_effects(
                capture_target_id,
                target_position,
                Some(object_id),
                true,
                false,
                effects,
            );
        }

        // C++ `triggerAbilityEffect` awards XP before the capture
        // switch (`SpecialAbilityUpdate.cpp:1248-1253`), including
        // garrison-evac triggers that do not defect the building.
        self.award_ability_trigger_experience(
            object_id,
            Self::award_xp_for_capture_trigger(capture_power),
        );
        // C++ `triggerAbilityEffect` always plays INI TriggerSound
        // (`SpecialAbilityUpdate.cpp:1267-1269`) before the switch.
        self.queue_capture_trigger_sound(object_id, capture_power);

        let did_capture = if self.can_unit_capture_building(object_id, capture_target_id, false) {
            let target_is_garrisonable = self
                .objects
                .get(&capture_target_id)
                .is_some_and(|target| target.thing().template.garrison_contain_max.is_some());
            if target_is_garrisonable {
                // C++ `removeAllContained(TRUE); break;`: clearing
                // a garrison is a successful ability trigger but
                // never defects that structure on the same use.
                self.evacuate_garrison_for_capture_with_effects(capture_target_id, effects)?;
                false
            } else {
                // C++ SpecialAbilityUpdate.cpp:1436-1442:
                // isLocallyControlled (owner player, not faction
                // Team) then defect. Leftover try_eva_building_stolen
                // already matches; call it before the flip so a
                // 2v2 same-faction ally victim stays silent.
                self.try_eva_building_stolen(capture_target_id);
                // SpecialAbilityUpdate.cpp:1442 defects to the
                // capturer's exact Team with one detection frame.
                // The receiving operation owns refund, timer,
                // capture callbacks and parking propagation.
                let destination = self
                    .objects
                    .get(&object_id)
                    .and_then(|capturer| capturer.team_instance_id);
                if let Some(team) = destination {
                    self.defect_host_object_with_effects(capture_target_id, team, 1, effects)?
                } else {
                    false
                }
            }
        } else {
            false
        };

        // `endPreparation` clears this status before C++ starts
        // PackTime. The source remains in the capture machine so
        // a completed capture is not reported as immediately idle.
        if let Some(object) = self.objects.get_mut(&object_id) {
            if object.is_alive() {
                object.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                    &mut self.queued_audio_events,
                ));
                object.set_status_using_ability(false);
                // C++ startPacking: clear UNPACKING|RAISING_FLAG, set PACKING.
                clear_raising_flag_model(object);
                object.capture_channel = Some(crate::game_logic::CaptureChannelState::new(
                    crate::game_logic::CaptureChannelPhase::Packing,
                    crate::game_logic::thing::vary_pack_unpack_duration_ms(
                        pack_time_ms,
                        object.thing().template.capture_pack_unpack_variation_factor,
                        &self.logic_random,
                    ),
                ));

                object.set_ai_state(
                    AIState::Capturing,
                    &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                );
            }
        }
        if self.objects.get(&object_id).is_some_and(|object| {
            object.is_alive()
                && object.capture_channel.map(|channel| channel.phase)
                    == Some(crate::game_logic::CaptureChannelPhase::Packing)
        }) {
            self.begin_capture_packing_pose(object_id, capture_power, true);
        }

        if pack_time_ms == 0 {
            self.finish_capture_channel(object_id)?;
        }

        if did_capture {
            // C++ Object::onCapture residual (kick/idle/AI-sell/deselect).
            // The receiving defect operation already ran onCapture.
            // C++ getAcademyStats()->recordBuildingCapture() residual.
            let player = match owner_player_id {
                Some(player_id) => self.get_player_mut(player_id),
                None => self.get_player_mut_by_team(team),
            };
            if let Some(p) = player {
                p.record_building_capture();
            }
            if capture_power == crate::game_logic::CapturePowerKind::BlackLotus {
                self.hero_abilities.record_building_capture();
            }

            if capture_power == crate::game_logic::CapturePowerKind::BlackLotus {
                if let Some(object) = self.objects.get_mut(&object_id) {
                    // C++ triggerAbilityEffect restarts only the
                    // Black Lotus capture timer here; infantry
                    // capture repeatedly reset it during prep.
                    object.start_power_recharge(&power_type);
                }
            }
            // C++ EVA_BuildingStolen already fired pre-flip via leftover
            // is_object_locally_controlled. Do not re-gate on faction Team.
            let msg = localization::localize("hud.capture.complete", "Building captured");
            self.queue_radar_message_for_team(team, msg);
        }
        Ok(())
    }
}
