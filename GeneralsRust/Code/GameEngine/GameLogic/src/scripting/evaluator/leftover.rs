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
                if !is_generals_challenge_campaign() {
                    return raw.to_string();
                }
                player_list()
                    .read()
                    .ok()
                    .and_then(|list| list.get_local_player().cloned())
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
            THE_PLAYER => {
                if !is_generals_challenge_campaign() {
                    raw.to_string()
                } else {
                    player_list()
                        .read()
                        .ok()
                        .and_then(|list| list.get_local_player().cloned())
                        .and_then(|p| {
                            p.read().ok().and_then(|p| {
                                NameKeyGenerator::key_to_name(p.get_player_name_key())
                            })
                        })
                        .unwrap_or_else(|| raw.to_string())
                }
            }
            THIS_PLAYER => self
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
        if kind_of_type_int < 0 {
            return None;
        }
        let bit = kind_of_type_int as u32;
        crate::common::ALL_KIND_OF
            .iter()
            .copied()
            .find(|kind| kind.cpp_bit() == Some(bit))
    }
}
