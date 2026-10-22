//! Optional authored terrain witness inside the existing real-window camera gate.
//! Observations are detached evidence, never a second mutable terrain authority.
use super::*;
use crate::game_logic::{NativeMapAdmission, NativeTerrainEffects};

#[derive(serde::Deserialize)]
pub(super) struct TerrainOwnerRequest {
    map: Value,
    map_ini: Value,
    private_userdata: PathBuf,
    expected_grid_cells: [u32; 2],
    expected_cell_size: f32,
    expected_origin: [f32; 3],
}
#[derive(Clone, Copy, Debug)]
struct NativeFrameWitness {
    frame: u64,
    total: u64,
    last: u64,
    active: bool,
}
impl NativeFrameWitness {
    fn report(self) -> Value {
        json!({"frame":self.frame,"total_owned_submits":self.total,
            "last_frame_submit_count":self.last,"frame_is_active":self.active})
    }
}
fn completed_owned_frame(before: NativeFrameWitness, after: NativeFrameWitness) -> bool {
    !before.active
        && !after.active
        && after.frame == before.frame.wrapping_add(1)
        && after.total == before.total.wrapping_add(1)
        && after.last == 1
}
struct TerrainObservation {
    logical_raw: Vec<u8>,
    native_raw: Vec<u8>,
    revision: u64,
    water: gamelogic::terrain_water::WaterGridState,
    scorches: Vec<game_client::terrain::scorch_mesh::ScorchMark>,
}
impl PartialEq for TerrainObservation {
    fn eq(&self, other: &Self) -> bool {
        self.revision == other.revision && self.content_equals(other)
    }
}
impl TerrainObservation {
    fn content_equals(&self, other: &Self) -> bool {
        self.logical_raw == other.logical_raw
            && self.native_raw == other.native_raw
            && self.report()["water"] == other.report()["water"]
            && self.report()["scorches"] == other.report()["scorches"]
    }
    fn report(&self) -> Value {
        let mut heights: Vec<_> = self.water.height_deltas.iter().collect();
        heights.sort_by_key(|(key, _)| **key);
        let mut motion: Vec<_> = self.water.mesh_motion.iter().collect();
        motion.sort_by_key(|(key, _)| **key);
        json!({"raw_count":self.logical_raw.len(),"logical_raw_crc32":format!("{:08x}",crc32fast::hash(&self.logical_raw)),
            "native_raw_crc32":format!("{:08x}",crc32fast::hash(&self.native_raw)),"height_revision":self.revision,
            "digest_scope":"CRC32 runtime observation; full byte vectors are compared in-process, parent pins archive SHA256",
            "water":{"enabled":self.water.enabled,"transform_bits":self.water.transform.to_cols_array().map(f32::to_bits),
                "resolution_bits":[self.water.resolution.0.to_bits(),self.water.resolution.1.to_bits(),self.water.resolution.2.to_bits()],
                "clamp_bits":[self.water.height_clamps.0.to_bits(),self.water.height_clamps.1.to_bits()],
                "attenuation_bits":[self.water.attenuation.0.to_bits(),self.water.attenuation.1.to_bits(),self.water.attenuation.2.to_bits(),self.water.attenuation.3.to_bits()],
                "height_deltas":heights.into_iter().map(|(&(x,y),v)|json!([x,y,v.to_bits()])).collect::<Vec<_>>(),
                "mesh_motion":motion.into_iter().map(|(&(x,y),v)|json!([x,y,v.velocity.to_bits(),v.status,v.preferred_height])).collect::<Vec<_>>()},
            "scorches":self.scorches.iter().map(|s|json!({"position_bits":s.location.map(f32::to_bits),"radius_bits":s.radius.to_bits(),"kind":s.scorch_type})).collect::<Vec<_>>()})
    }
}
pub(super) struct TerrainGateState {
    saved: TerrainObservation,
    first_restored: Option<TerrainObservation>,
    evidence: Value,
}
impl CnCGameEngine {
    pub(super) fn terrain_gate_startup_paths(
        &mut self,
        request: &TerrainOwnerRequest,
    ) -> Result<Value, String> {
        let root = request
            .private_userdata
            .canonicalize()
            .map_err(|e| e.to_string())?;
        let options = self
            .ui_manager
            .options_preferences_mut()
            .configured_path()
            .ok_or("receiving camera options have no admitted filename")?
            .to_owned();
        let saves = self.save_file_manager.save_directory().to_owned();
        let replays = self.receiving_replay_menu_inputs().replay_directory;
        let campaign = crate::save_load::game_state::with_global_campaign_manager(|manager| {
            manager.campaign_directory().to_owned()
        })
        .map_err(str::to_owned)?;
        for path in [&options, &saves, &replays, &campaign] {
            require(
                path.is_absolute()
                    && path.starts_with(&root)
                    && !path
                        .components()
                        .any(|component| matches!(component, std::path::Component::ParentDir)),
                "actual startup writer is outside explicit private userdata",
            )?;
            let mut existing = path.as_path();
            while !existing.exists() {
                existing = existing
                    .parent()
                    .ok_or("writer path has no existing private ancestor")?;
            }
            require(
                existing
                    .canonicalize()
                    .map_err(|e| e.to_string())?
                    .starts_with(&root),
                "startup writer ancestor escapes through symlink",
            )?;
        }
        Ok(
            json!({"private_userdata":root,"camera_preferences":options,"save_directory":saves,"recorder_replays":replays,
            "campaign_directory":campaign,"scope":"actual receiving application paths; CampaignManager remains existing application singleton, not second-Engine campaign isolation"}),
        )
    }
    fn terrain_gate_observe(&self) -> Result<TerrainObservation, String> {
        let terrain = self
            .game_logic
            .world_services
            .terrain()
            .read()
            .map_err(|_| "receiving terrain poisoned")?;
        require(
            terrain.has_height_map(),
            "actual loaded receiving terrain has no height grid",
        )?;
        let (width, height) = terrain.logic_height_map_extents();
        let logical_raw = terrain.logic_height_map_bytes().to_vec();
        let visual = self
            .render_pipeline
            .native_terrain()
            .ok_or("actual pipeline has no native terrain owner")?;
        let mut native_raw = Vec::with_capacity(logical_raw.len());
        for y in 0..height {
            for x in 0..width {
                native_raw.push(
                    u8::try_from(visual.get_raw_map_height(
                        x - terrain.logic_height_map_border_size(),
                        y - terrain.logic_height_map_border_size(),
                    ))
                    .map_err(|_| "native height not an accepted byte")?,
                );
            }
        }
        require(
            native_raw == logical_raw,
            "receiving native raw grid differs from canonical logical bytes",
        )?;
        Ok(TerrainObservation {
            logical_raw,
            native_raw,
            revision: terrain.height_revision(),
            water: visual.capture_water_state(),
            scorches: visual.scorch_marks().to_vec(),
        })
    }
    pub(super) fn terrain_gate_seed(
        &mut self,
        request: &TerrainOwnerRequest,
        map: &Path,
        paths: Value,
    ) -> Result<TerrainGateState, String> {
        require(
            file_observation(map)? == request.map,
            "authored terrain map request differs",
        )?;
        let ini = map.parent().ok_or("map parent missing")?.join("Map.ini");
        require(
            file_observation(&ini)? == request.map_ini,
            "authored companion Map.ini request differs",
        )?;
        let mut terrain = self
            .game_logic
            .world_services
            .terrain()
            .write()
            .map_err(|_| "receiving terrain poisoned")?;
        let grid = terrain.water_grid_state();
        require(
            grid.enabled,
            "real map definitions did not enable receiving water grid",
        )?;
        require(
            [grid.resolution.0, grid.resolution.1] == request.expected_grid_cells.map(|n| n as f32)
                && grid.resolution.2.to_bits() == request.expected_cell_size.to_bits(),
            "actual admitted grid dimensions differ from authored content",
        )?;
        let origin = grid.transform.w_axis.truncate().to_array();
        require(
            origin.map(f32::to_bits) == request.expected_origin.map(f32::to_bits),
            "actual admitted water origin differs",
        )?;
        let (width, height) = terrain.logic_height_map_extents();
        let border = terrain.logic_height_map_border_size();
        let selected = (1..height - 2 * border - 1)
            .flat_map(|y| (1..width - 2 * border - 1).map(move |x| (x, y)))
            .find(|&(x, y)| {
                terrain.logic_height_map_bytes()[((y + border) * width + x + border) as usize] > 0
            })
            .ok_or("no valid positive interior raw sample")?;
        let old = terrain.logic_height_map_bytes()
            [((selected.1 + border) * width + selected.0 + border) as usize];
        let visual = self
            .render_pipeline
            .native_terrain_mut()
            .ok_or("native terrain unavailable")?;
        require(
            visual.capture_water_state() == *grid,
            "native grid was not admitted from actual receiving map definitions",
        )?;
        let initial_scorches = visual.scorch_marks().to_vec();
        let mut edits = 0;
        terrain.set_raw_map_height_with_effects(
            selected.0,
            selected.1,
            i32::from(old) - 1,
            &mut |change| {
                edits += 1;
                visual.accepted_height(change);
            },
        );
        require(
            edits == 1,
            "raw edit did not produce exactly one synchronous native effect",
        )?;
        require(
            visual.scorch_marks() == initial_scorches,
            "height edit erased authored scorches",
        )?;
        drop(terrain);
        let [x, y, z] = request.expected_origin;
        visual.add_water_velocity(
            x + request.expected_cell_size,
            y + request.expected_cell_size,
            3.0,
            z + 4.0,
        );
        let water = visual.capture_water_state();
        require(
            !water.mesh_motion.is_empty() && water.mesh_motion.values().any(|m| m.velocity != 0.0),
            "actual native water velocity admission had no effect",
        )?;
        NativeTerrainEffects::add_scorch(visual, [x, y, z], 5.0, 0);
        require(
            visual.scorch_marks().len() == initial_scorches.len() + 1
                && visual.scorch_marks().starts_with(&initial_scorches),
            "scorch append lost original order",
        )?;
        let saved = self.terrain_gate_observe()?;
        let evidence = json!({"scope":"authored-native-window","startup_paths":paths,"configured_grid_source_hashes":{"scope":"runtime CRC observations; parent launch receipt independently pins map and Map.ini SHA256","map":request.map,"Map.ini":request.map_ini},
            "accepted_raw_edit":{"playable_sample":selected,"full_grid_sample":[selected.0+border,selected.1+border],"old":old,"requested":i32::from(old)-1,"accepted":old-1,"effects":edits},"saved":saved.report(),"saved_raw_digest":saved.report()["logical_raw_crc32"],"saved_water_full_status_and_motion":saved.report()["water"],"saved_scorches_order":saved.report()["scorches"],
            "leaf_scope":"real accepted-byte delivery, native water velocity and scorch leaf; not complete construction/combat gameplay"});
        Ok(TerrainGateState {
            saved,
            first_restored: None,
            evidence,
        })
    }
    pub(super) fn terrain_gate_restore_and_hydrate(
        &mut self,
        state: &mut TerrainGateState,
    ) -> Result<(), String> {
        let restored = self.terrain_gate_observe()?;
        require(
            restored.content_equals(&state.saved),
            "actual host restore lost receiving raw/water/scorch state",
        )?;
        self.render_pipeline
            .hydrate_native_terrain_gpu(
                &self.graphics_system.device_arc(),
                &self.graphics_system.queue_arc(),
            )
            .map_err(|e| e.to_string())?;
        let hydrated = self.terrain_gate_observe()?;
        require(
            hydrated == restored,
            "actual GPU hydration reconstructed or mutated receiving CPU terrain state",
        )?;
        state.evidence["first_restore_equal"] = json!(true);
        state.evidence["native_before_and_after_gpu_hydration"] =
            json!({"before":restored.report(),"after":hydrated.report(),"equal":true});
        state.first_restored = Some(hydrated);
        Ok(())
    }
    pub(super) fn terrain_gate_mutate_peer(
        &self,
        peer: &mut StagedRestoreWorld,
    ) -> Result<(), String> {
        let origin = {
            let terrain = peer
                .logic
                .world_services
                .terrain()
                .read()
                .map_err(|_| "peer terrain poisoned")?;
            terrain
                .water_grid_state()
                .transform
                .w_axis
                .truncate()
                .to_array()
        };
        let receiver = peer
            .native_map
            .terrain_effects()
            .ok_or("staged peer has no native terrain owner")?;
        receiver.add_water_velocity(origin[0] + 10.0, origin[1] + 10.0, 1.0, origin[2] + 4.0);
        receiver.add_scorch(origin, 2.0, 0);
        Ok(())
    }
    pub(super) fn terrain_gate_peer_and_failed_archive(
        &mut self,
        state: &mut TerrainGateState,
        slot: &str,
    ) -> Result<(), String> {
        let after_peer = self.terrain_gate_observe()?;
        require(
            state
                .first_restored
                .as_ref()
                .is_some_and(|before| before == &after_peer),
            "staged peer/reset changed receiving terrain owner or height revision",
        )?;
        state.evidence["staged_peer_receiver_unchanged"] = json!(true);
        let camera = logic_camera(&self.game_logic)?;
        let corrupt = "terrain_owner_truncated";
        let path = self.save_file_manager.get_save_path(corrupt);
        require(!path.exists(), "failed-archive slot must be fresh")?;
        let bytes =
            std::fs::read(self.save_file_manager.get_save_path(slot)).map_err(|e| e.to_string())?;
        require(
            bytes.len() > 32,
            "saved archive too small for bounded truncation",
        )?;
        std::fs::write(&path, &bytes[..bytes.len() / 2]).map_err(|e| e.to_string())?;
        require(
            self.host_load_game_authority(corrupt).is_err(),
            "truncated archive unexpectedly accepted",
        )?;
        require(
            self.terrain_gate_observe()? == after_peer && logic_camera(&self.game_logic)? == camera,
            "failed archive changed receiving camera/raw/water/scorch/revision",
        )?;
        state.evidence["failed_archive_receiver_unchanged"] = json!(true);
        state.evidence["failure_scope"] = json!(
            "actual truncated archive decode failure, not every possible postcommit Client error"
        );
        Ok(())
    }
    pub(super) fn terrain_gate_finish(
        &mut self,
        mut state: TerrainGateState,
        attempted_frame: &mut Value,
    ) -> Result<Value, String> {
        // All camera/HAG comparisons have completed before this extra real frame.
        let before = self.terrain_gate_observe()?;
        require(
            before.content_equals(&state.saved),
            "later camera save/load changed terrain continuation",
        )?;
        let observe = |engine: &Self| NativeFrameWitness {
            frame: engine.graphics_system.get_statistics().frame_count,
            total: ww3d_engine::total_owned_submits(),
            last: ww3d_engine::last_frame_submit_count(),
            active: ww3d_engine::frame_is_active(),
        };
        let prior = observe(self);
        *attempted_frame = json!({"before":prior.report(),"terrain_before":before.report(),
            "status":"render_attempt_started"});
        let render_result = self.render().map_err(|error| error.to_string());
        let actual = observe(self);
        // Preserve the actual counters even when render or a strict assertion fails.
        *attempted_frame = json!({"before":prior.report(),"after":actual.report(),
            "terrain_before":before.report(),"render_returned_ok":render_result.is_ok(),
            "render_error":render_result.as_ref().err(),
            "completed_owned_frame":completed_owned_frame(prior,actual)});
        render_result?;
        require(
            actual.frame == prior.frame.wrapping_add(1),
            "actual native render did not execute a new frame",
        )?;
        require(
            actual.total == prior.total.wrapping_add(1) && actual.last == 1,
            "actual native render did not submit exactly one new owned frame",
        )?;
        require(
            !prior.active && !actual.active,
            "actual native render entered or left an active frame loan",
        )?;
        state.evidence["actual_native_frame"] = json!({"render_returned_ok":true,
            "frame_before":prior.frame,"frame_after":actual.frame,
            "owned_frame_submit_count":actual.last,"attempt":attempted_frame,
            "before":before.report(),"after":self.terrain_gate_observe()?.report(),
            "scope":"normal actual Engine render after all existing HAG assertions; native water may advance normally"});
        Ok(state.evidence)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owned_frame_witness_rejects_stale_duplicate_and_active_submissions() {
        let before = NativeFrameWitness {
            frame: 7,
            total: 12,
            last: 1,
            active: false,
        };
        let real = NativeFrameWitness {
            frame: 8,
            total: 13,
            last: 1,
            active: false,
        };
        assert!(completed_owned_frame(before, real));
        assert!(
            !completed_owned_frame(before, NativeFrameWitness { total: 12, ..real }),
            "stale last=1 cannot prove a new submit"
        );
        assert!(
            !completed_owned_frame(before, NativeFrameWitness { total: 14, ..real }),
            "two submits cannot impersonate one owned frame"
        );
        assert!(!completed_owned_frame(
            before,
            NativeFrameWitness {
                active: true,
                ..real
            }
        ));
        assert!(!completed_owned_frame(
            NativeFrameWitness {
                active: true,
                ..before
            },
            real
        ));
        assert!(!completed_owned_frame(
            before,
            NativeFrameWitness { frame: 7, ..real }
        ));
        assert!(!completed_owned_frame(
            before,
            NativeFrameWitness { last: 0, ..real }
        ));
        assert!(completed_owned_frame(
            NativeFrameWitness {
                frame: u64::MAX,
                total: u64::MAX,
                ..before
            },
            NativeFrameWitness {
                frame: 0,
                total: 0,
                ..real
            },
        ));
    }

    #[test]
    fn absent_terrain_request_preserves_existing_camera_only_wire_admission() {
        let request: CameraGateRequest = serde_json::from_value(json!({
            "nonce":"0".repeat(64),"pid":1,"report_path":"/acceptance/report.json",
            "actual_executable":null,"map":null,"build_receipt":null
        }))
        .unwrap();
        assert!(request.terrain_owner.is_none());
    }

    #[test]
    fn terrain_window_observation_compares_complete_water_bits_and_scorch_order() {
        let make = || TerrainObservation {
            logical_raw: vec![40, 7],
            native_raw: vec![40, 7],
            revision: 3,
            water: Default::default(),
            scorches: Vec::new(),
        };
        let mut before = make();
        let mut after = make();
        before.water.mesh_motion.insert(
            (1, 2),
            gamelogic::terrain_water::WaterGridMeshMotion {
                velocity: -0.0,
                status: 0x82,
                preferred_height: 17,
            },
        );
        after.water = before.water.clone();
        assert!(before == after);
        after.water.mesh_motion.get_mut(&(1, 2)).unwrap().velocity = 0.0;
        assert!(before != after, "full source float bits must be observed");
        after.water = before.water.clone();
        after.water.mesh_motion.get_mut(&(1, 2)).unwrap().status = 0;
        assert!(before != after, "upper source status bits cannot disappear");
        after.water = before.water.clone();
        let mark = game_client::terrain::scorch_mesh::ScorchMark {
            location: [1., 2., 3.],
            radius: 5.,
            scorch_type: 0,
        };
        before.scorches = vec![
            mark,
            game_client::terrain::scorch_mesh::ScorchMark { radius: 6., ..mark },
        ];
        after.scorches = before.scorches.iter().copied().rev().collect();
        assert!(before != after, "saved scorch order is observable");
    }
}
