// Evaluator resolve helpers leftover from family splits
//
// Split from `scripting/evaluator.rs` for module-size parity.
// Observable behavior is unchanged.

impl ScriptEvaluator {
    fn resolve_team_name_token(&self, raw: &str) -> String {
        match raw {
            THIS_TEAM => self
                .with_evaluation_engine_ref(|engine| {
                    engine
                        .get_condition_team_name()
                        .or_else(|| engine.get_calling_team_name())
                })
                .flatten()
                .unwrap_or_else(|| raw.to_string()),
            TEAM_THE_PLAYER => {
                let current_player = self
                    .with_evaluation_engine_ref(|engine| engine.get_current_player_name())
                    .flatten();
                let Some(player_name) = current_player else {
                    return raw.to_string();
                };

                player_list()
                    .read()
                    .ok()
                    .and_then(|list| list.find_player_by_name(&player_name))
                    .and_then(|p| p.read().ok().and_then(|p| p.get_default_team()))
                    .and_then(|team| team.read().ok().map(|t| t.get_name().to_string()))
                    .unwrap_or_else(|| raw.to_string())
            }
            _ => raw.to_string(),
        }
    }

    fn resolve_team_instances(&self, team_name: &str) -> Vec<Arc<RwLock<crate::team::Team>>> {
        let Ok(mut factory) = get_team_factory().lock() else {
            return Vec::new();
        };

        if let Some(team_arc) = factory.find_team(team_name) {
            return vec![team_arc];
        }

        factory.find_team_instances(team_name)
    }

    fn resolve_object_types(&self, param: &Parameter) -> ObjectTypes {
        let type_name = param.get_string();
        let mut types = ObjectTypes::new();
        if type_name.is_empty() {
            return types;
        }

        if let Some(Some(found)) =
            self.with_evaluation_engine_ref(|engine| engine.get_object_types(type_name))
        {
            return found;
        }

        types.add_object_type(AsciiString::from(type_name));
        types
    }

    fn resolve_player_from_param(
        &self,
        param: &Parameter,
    ) -> Option<Arc<RwLock<crate::player::Player>>> {
        let mask_bits = param.get_int() as u32;
        if param.get_parameter_type() == ParameterType::Side && mask_bits != 0 {
            let mask = PlayerMaskType::from_bits_truncate(mask_bits);
            if let Ok(list) = player_list().read() {
                for player_arc in list.iter() {
                    let Ok(player_guard) = player_arc.read() else {
                        continue;
                    };
                    if mask.intersects(player_guard.get_player_mask()) {
                        return Some(Arc::clone(player_arc));
                    }
                }
            }
        }

        let raw = param.get_string();
        let resolved = match raw {
            THE_PLAYER | THIS_PLAYER => self
                .with_evaluation_engine_ref(|engine| engine.get_current_player_name())
                .flatten()
                .unwrap_or_else(|| raw.to_string()),
            LOCAL_PLAYER => player_list()
                .read()
                .ok()
                .and_then(|list| list.get_local_player().cloned())
                .and_then(|p| {
                    p.read()
                        .ok()
                        .and_then(|p| NameKeyGenerator::key_to_name(p.get_player_name_key()))
                })
                .unwrap_or_else(|| raw.to_string()),
            _ => raw.to_string(),
        };

        if resolved.is_empty() {
            return None;
        }
        player_list()
            .read()
            .ok()
            .and_then(|list| list.find_player_by_name(&resolved))
    }

    fn get_trigger_area(&self, area_name: &str) -> Option<PolygonTrigger> {
        if let Some(trigger) = self
            .with_evaluation_engine_ref(|engine| engine.get_qualified_trigger_area_by_name(area_name))
            .flatten()
        {
            return Some(trigger);
        }
        let resolved = crate::scripting::engine::qualify_trigger_area_name(area_name, None)?;
        let Ok(terrain) = get_terrain_logic().read() else {
            return None;
        };
        terrain.get_trigger_area_by_name(&resolved).cloned()
    }

    fn is_object_considerable(obj: &crate::object::Object) -> bool {
        if obj.is_effectively_dead() {
            return false;
        }
        !obj.is_kind_of(KindOf::Inert)
    }

    fn counts_for_unit_type_area_condition(
        is_effectively_dead: bool,
        is_inert: bool,
        is_crate: bool,
    ) -> bool {
        !(is_effectively_dead || is_inert) || is_crate
    }

    fn is_object_inside_trigger(obj: &crate::object::Object, trigger: &PolygonTrigger) -> bool {
        let pos = obj.get_position();
        let point = crate::common::ICoord3D::new(pos.x as i32, pos.y as i32, pos.z as i32);
        trigger.point_in_trigger_int(&point)
    }

    fn kind_of_type_to_mask(kind_of_type_int: i32) -> Option<KindOf> {
        match kind_of_type_int {
            0 => Some(KindOf::Obstacle),
            1 => Some(KindOf::Selectable),
            2 => Some(KindOf::Immobile),
            3 => Some(KindOf::CanAttack),
            4 => Some(KindOf::StickToTerrainSlope),
            5 => Some(KindOf::CanCastReflections),
            6 => Some(KindOf::Shrubbery),
            7 => Some(KindOf::Structure),
            8 => Some(KindOf::Infantry),
            9 => Some(KindOf::Vehicle),
            10 => Some(KindOf::Aircraft),
            11 => Some(KindOf::HugeVehicle),
            12 => Some(KindOf::Dozer),
            13 => Some(KindOf::Harvester),
            14 => Some(KindOf::CommandCenter),
            15 => Some(KindOf::Prison),
            16 => Some(KindOf::CollectsPrisonBounty),
            17 => Some(KindOf::PowTruck),
            18 => Some(KindOf::LineBuild),
            19 => Some(KindOf::Salvager),
            20 => Some(KindOf::WeaponSalvager),
            21 => Some(KindOf::Transport),
            22 => Some(KindOf::Bridge),
            23 => Some(KindOf::LandmarkBridge),
            24 => Some(KindOf::BridgeTower),
            25 => Some(KindOf::Projectile),
            26 => Some(KindOf::Preload),
            27 => Some(KindOf::NoGarrison),
            28 => Some(KindOf::WaveGuide),
            29 => Some(KindOf::WaveEffect),
            30 => Some(KindOf::NoCollide),
            31 => Some(KindOf::RepairPad),
            32 => Some(KindOf::HealPad),
            33 => Some(KindOf::StealthGarrison),
            34 => Some(KindOf::CashGenerator),
            35 => Some(KindOf::DrawableOnly),
            36 => Some(KindOf::CountsForVictory),
            37 => Some(KindOf::RebuildHole),
            38 => Some(KindOf::Score),
            39 => Some(KindOf::ScoreCreate),
            40 => Some(KindOf::ScoreDestroy),
            41 => Some(KindOf::NoHealIcon),
            42 => Some(KindOf::CanRappel),
            43 => Some(KindOf::Parachutable),
            44 => Some(KindOf::CanSurrender),
            45 => Some(KindOf::CanBeRepulsed),
            46 => Some(KindOf::MobNexus),
            47 => Some(KindOf::IgnoredInGui),
            48 => Some(KindOf::Crate),
            49 => Some(KindOf::Capturable),
            50 => Some(KindOf::ClearedByBuild),
            51 => Some(KindOf::SmallMissile),
            52 => Some(KindOf::AlwaysVisible),
            53 => Some(KindOf::Unattackable),
            54 => Some(KindOf::Mine),
            55 => Some(KindOf::CleanupHazard),
            56 => Some(KindOf::PortableStructure),
            57 => Some(KindOf::AlwaysSelectable),
            58 => Some(KindOf::AttackNeedsLineOfSight),
            59 => Some(KindOf::WalkOnTopOfWall),
            60 => Some(KindOf::DefensiveWall),
            61 => Some(KindOf::FSPower),
            64 => Some(KindOf::FSTechnology),
            65 => Some(KindOf::AircraftPathAround),
            66 => Some(KindOf::LowOverlappable),
            67 => Some(KindOf::ForceAttackable),
            68 => Some(KindOf::AutoRallypoint),
            69 => Some(KindOf::TechBuilding),
            70 => Some(KindOf::Powered),
            71 => Some(KindOf::ProducedAtHelipad),
            72 => Some(KindOf::Drone),
            74 => Some(KindOf::BallisticMissile),
            75 => Some(KindOf::ClickThrough),
            76 => Some(KindOf::SupplySourceOnPreview),
            78 => Some(KindOf::GarrisonableUntilDestroyed),
            79 => Some(KindOf::Boat),
            80 => Some(KindOf::ImmuneToCapture),
            81 => Some(KindOf::Hulk),
            82 => Some(KindOf::ShowPortraitWhenControlled),
            83 => Some(KindOf::SpawnsAreTheWeapons),
            84 => Some(KindOf::CannotBuildNearSupplies),
            85 => Some(KindOf::SupplySource),
            86 => Some(KindOf::RevealToAll),
            87 => Some(KindOf::Disguiser),
            88 => Some(KindOf::Inert),
            89 => Some(KindOf::Hero),
            90 => Some(KindOf::IgnoresSelectAll),
            91 => Some(KindOf::DontAutoCrushInfantry),
            92 => Some(KindOf::CliffJumper),
            93 => Some(KindOf::FSSupplyDropzone),
            94 => Some(KindOf::FSSuperweapon),
            95 => Some(KindOf::FsBlackMarket),
            96 => Some(KindOf::FSSupplyCenter),
            97 => Some(KindOf::FSStrategyCenter),
            98 => Some(KindOf::MoneyHacker),
            99 => Some(KindOf::ArmorSalvager),
            100 => Some(KindOf::RevealsEnemyPaths),
            101 => Some(KindOf::BoobyTrap),
            102 => Some(KindOf::FSFake),
            103 => Some(KindOf::FSInternetCenter),
            104 => Some(KindOf::BlastCrater),
            _ => None,
        }
    }
}
