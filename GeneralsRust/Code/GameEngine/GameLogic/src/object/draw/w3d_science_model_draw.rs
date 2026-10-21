use super::draw_module::*;
use super::w3d_model_draw::*;
use crate::common::*;
use crate::player::ThePlayerList;
use game_engine::common::ini::{INI, INIError};
use game_engine::common::rts::{SCIENCE_INVALID, ScienceType, get_science_store};
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType, TimeOfDay};
use std::any::Any;

/// The full local player's science membership frozen by the receiving world.
/// Shared immutable frame data; actual modules retain only their resulting bool.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ReceivingSciencePlayer {
    active: bool,
    sciences: Box<[ScienceType]>,
}
impl ReceivingSciencePlayer {
    pub fn new(active: bool, sciences: impl IntoIterator<Item = ScienceType>) -> Self {
        let mut sciences = sciences.into_iter().collect::<Vec<_>>();
        sciences.sort_unstable();
        sciences.dedup();
        Self {
            active,
            sciences: sciences.into_boxed_slice(),
        }
    }
    /// W3DScienceModelDraw.cpp50-70: invalid always hides, inactive observers draw.
    pub fn should_hide(&self, required: ScienceType) -> bool {
        required == SCIENCE_INVALID
            || (self.active && self.sciences.binary_search(&required).is_err())
    }
}

#[derive(Debug, Clone)]
pub struct W3DScienceModelDrawModuleData {
    pub base: W3DModelDrawModuleData,
    pub required_science: ScienceType,
}

impl W3DScienceModelDrawModuleData {
    pub fn new() -> Self {
        Self {
            base: W3DModelDrawModuleData::new(),
            required_science: SCIENCE_INVALID,
        }
    }

    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        loop {
            ini.read_line()?;
            if ini.is_eof() {
                return Err(INIError::EndOfFile);
            }
            let tokens = ini
                .get_buffer()
                .split(|character: char| character.is_ascii_whitespace() || character == '=')
                .filter(|token| !token.is_empty())
                .map(str::to_owned)
                .collect::<Vec<_>>();
            let Some(key) = tokens.first().cloned() else {
                continue;
            };
            if key.eq_ignore_ascii_case("End") {
                break;
            }
            let value_tokens = tokens
                .iter()
                .map(String::as_str)
                .skip(1)
                .filter(|t| *t != "=")
                .collect::<Vec<_>>();
            match key.to_ascii_uppercase().as_str() {
                "REQUIREDSCIENCE" => {
                    let name = required_value(&value_tokens)?;
                    self.required_science = if let Some(facts) = ini.receiving_science_names() {
                        facts.science(name).ok_or(INIError::InvalidData)?
                    } else {
                        // Explicit standalone parser retains application compatibility.
                        let store = get_science_store().ok_or(INIError::InvalidData)?;
                        store.get_science_from_internal_name(name)
                    };
                }
                _ => {
                    if !self
                        .base
                        .parse_ini_field(ini, key.as_str(), &value_tokens)?
                    {
                        return Err(INIError::UnknownToken);
                    }
                }
            }
        }
        Ok(())
    }
}

fn required_value<'a>(tokens: &'a [&str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|t| !t.is_empty())
        .ok_or(INIError::InvalidData)
}

impl Default for W3DScienceModelDrawModuleData {
    fn default() -> Self {
        Self::new()
    }
}
impl ModuleData for W3DScienceModelDrawModuleData {
    fn get_minimum_required_game_lod(
        &self,
    ) -> game_engine::common::thing::module::StaticGameLodLevel {
        self.base.get_minimum_required_game_lod()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.base.set_module_tag_name_key(key);
    }
    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.base.get_module_tag_name_key()
    }
}
impl DrawModuleData for W3DScienceModelDrawModuleData {
    fn as_any(&self) -> &dyn Any {
        self
    }
}
impl Snapshotable for W3DScienceModelDrawModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.crc(xfer)
    }
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.xfer(xfer)
    }
    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()
    }
}

pub struct W3DScienceModelDraw {
    data: W3DScienceModelDrawModuleData,
    base: W3DModelDraw,
    // Runtime receiving observation, rebound before the actual draw; not Xfer state.
    receiving_hidden: Option<bool>,
}
impl W3DScienceModelDraw {
    pub fn new(data: W3DScienceModelDrawModuleData) -> Self {
        Self {
            base: W3DModelDraw::new(data.base.clone()),
            data,
            receiving_hidden: None,
        }
    }
    /// Last frozen receiving decision; applying hidden remains a draw-phase effect.
    pub fn receiving_science_hidden(&self) -> Option<bool> {
        self.receiving_hidden
    }
    pub fn bind_receiving_science_player(
        &mut self,
        player: Option<&ReceivingSciencePlayer>,
    ) -> Result<(), &'static str> {
        // C++ rejects invalid data before it queries any local player.
        let hidden = if self.data.required_science == SCIENCE_INVALID {
            true
        } else {
            player
                .ok_or("valid authored science requires its receiving local player")?
                .should_hide(self.data.required_science)
        };
        self.receiving_hidden = Some(hidden);
        Ok(())
    }
    pub fn bind_owner_id(&mut self, owner_id: ObjectID) {
        self.base.bind_owner_id(owner_id);
    }
}
impl Module for W3DScienceModelDraw {
    fn on_object_created(&mut self) {
        self.base.on_object_created();
    }
    fn on_drawable_bound_to_object(&mut self) {
        self.base.on_drawable_bound_to_object();
    }
    fn preload_assets(&mut self, time_of_day: TimeOfDay) {
        self.base.preload_assets(time_of_day);
    }
    fn on_delete(&mut self) {
        self.base.on_delete();
    }
    fn get_module_name_key(&self) -> NameKeyType {
        game_engine::common::name_key_generator::NameKeyGenerator::name_to_key(
            "W3DScienceModelDraw",
        )
    }

    fn get_module_name_key_with_namespace(
        &self,
        keys: &mut game_engine::common::name_key_generator::NameKeyNamespace,
    ) -> NameKeyType {
        keys.name_to_key("W3DScienceModelDraw")
    }
    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.base.get_module_tag_name_key()
    }
    fn get_module_data(&self) -> &dyn ModuleData {
        &self.data
    }
}
impl DrawModule for W3DScienceModelDraw {
    fn on_drawable_bound_to_object_with_context(
        &mut self,
        binding: &DrawModuleBindingContext,
        drawable: &ModelDrawContext,
    ) -> bool {
        let handled = self
            .base
            .on_drawable_bound_to_object_with_context(binding, drawable);
        handled
    }

    fn do_draw_module(
        &mut self,
        transform_mtx: &Matrix3D,
        time: &crate::helpers::DrawTimeObservation,
    ) {
        self.do_draw_module_for_owner(transform_mtx, None, time);
    }

    fn do_draw_module_for_owner(
        &mut self,
        transform_mtx: &Matrix3D,
        driver: Option<&crate::object::drawable::DrawableRenderOwner<'_>>,
        time: &crate::helpers::DrawTimeObservation,
    ) {
        let hidden = self
            .receiving_hidden
            .unwrap_or_else(|| leftover_science_model_should_hide(self.data.required_science));
        if hidden {
            DrawModule::set_hidden(self, true);
            return;
        }
        self.base
            .do_draw_module_for_owner(transform_mtx, driver, time);
    }
    fn set_shadows_enabled(&mut self, enable: bool) {
        self.base.set_shadows_enabled(enable);
    }
    fn release_shadows(&mut self) {
        self.base.release_shadows();
    }
    fn allocate_shadows(&mut self) {
        self.base.allocate_shadows();
    }
    fn set_fully_obscured_by_shroud(&mut self, fully_obscured: bool) {
        self.base.set_fully_obscured_by_shroud(fully_obscured);
    }
    fn set_hidden(&mut self, hidden: bool) {
        DrawModule::set_hidden(&mut self.base, hidden);
    }
    fn is_visible(&self) -> bool {
        self.base.is_visible()
    }
    fn react_to_transform_change(
        &mut self,
        old_mtx: &Matrix3D,
        old_pos: &Coord3D,
        old_angle: Real,
    ) {
        self.base
            .react_to_transform_change(old_mtx, old_pos, old_angle);
    }
    fn react_to_geometry_change(&mut self) {
        self.base.react_to_geometry_change();
    }
    fn get_object_draw_interface(&self) -> Option<&dyn ObjectDrawInterface> {
        Some(&self.base)
    }
    fn get_object_draw_interface_mut(&mut self) -> Option<&mut dyn ObjectDrawInterface> {
        Some(&mut self.base)
    }
}
impl Snapshotable for W3DScienceModelDraw {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.crc(xfer)
    }
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;
        self.base.xfer(xfer)
    }
    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()
    }
}

/// C++ `W3DScienceModelDraw::doDrawModule` hide gate.
/// Invalid science or an active local player lacking it hides the mesh.
/// Observers (inactive local player) still draw. Missing leftover player list
/// fails open so observers/headless keep drawing.
pub fn leftover_science_model_should_hide(science: ScienceType) -> bool {
    if science == SCIENCE_INVALID {
        return true;
    }
    let has_science = ThePlayerList()
        .read()
        .ok()
        .and_then(|list| list.get_local_player().cloned())
        .and_then(|p| {
            p.read()
                .ok()
                .map(|g| (!g.is_player_active()) || g.has_science(science))
        })
        .unwrap_or(true);
    !has_science
}

/// Leftover INI `RequiredScience` for a live presentation template.
pub fn leftover_science_model_data(template_name: &str) -> Option<W3DScienceModelDrawModuleData> {
    if template_name.is_empty() {
        return None;
    }
    let Ok(guard) = game_engine::common::thing::get_thing_factory() else {
        return None;
    };
    let factory = guard.as_ref()?;
    let template = factory.find_template(template_name, false)?;
    for entry in template.get_draw_module_info().iter() {
        if let Some(data) = entry
            .data
            .as_any()
            .downcast_ref::<W3DScienceModelDrawModuleData>()
        {
            return Some(data.clone());
        }
    }
    None
}

/// Leftover-tick `W3DScienceModelDraw::doDrawModule` hide residual for live host.
pub fn tick_live_host_science_model_hide(
    template_name: &str,
    data: Option<&W3DScienceModelDrawModuleData>,
) -> bool {
    let module_data = data
        .cloned()
        .or_else(|| leftover_science_model_data(template_name))
        .unwrap_or_else(W3DScienceModelDrawModuleData::new);
    let hide = leftover_science_model_should_hide(module_data.required_science);
    let mut draw = W3DScienceModelDraw::new(module_data);
    // This unbound hide probe has no receiving View. Science/model draws do
    // not consume camera freeze; vehicle draw entrypoints require real facts.
    draw.do_draw_module(
        &Matrix3D::IDENTITY,
        &crate::helpers::DrawTimeObservation::new(None, false, false, true),
    );
    hide
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::common::name_key_generator::NameKeyGenerator;

    #[test]
    fn receiving_science_gate_preserves_original_initial_hidden_and_observer_cases() {
        // Actual original W3DScienceModelDraw.cpp50-70 component outputs. The
        // component's base is a phase witness, not a replacement renderer.
        let rows = include_str!("fixtures/science_model_gate_cpp.jsonl")
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(rows.len(), 16);
        for row in rows {
            let tag = row["tag"].as_str().unwrap();
            let fields = tag.split('_').collect::<Vec<_>>();
            let required = fields[0][1..].parse::<i32>().unwrap();
            let active = fields[1] == "a1";
            let owns = fields[2] == "h1";
            let initial_hidden = fields[3] == "initial1";
            let observation = ReceivingSciencePlayer::new(active, owns.then_some(7));
            let hide = !row["base_called"].as_bool().unwrap();
            assert_eq!(observation.should_hide(required), hide, "{tag}");
            let mut data = W3DScienceModelDrawModuleData::new();
            data.required_science = required;
            let mut state = ModelConditionInfo::new();
            state.model_name = AsciiString::from("ReceivingScienceProbe");
            data.base.condition_states.push(state);
            data.base.default_state = 0;
            let mut draw = W3DScienceModelDraw::new(data);
            Module::on_drawable_bound_to_object(&mut draw);
            DrawModule::set_hidden(&mut draw, initial_hidden);
            draw.bind_receiving_science_player(Some(&observation))
                .unwrap();
            DrawModule::do_draw_module(
                &mut draw,
                &Matrix3D::IDENTITY,
                &crate::helpers::DrawTimeObservation::new(None, false, false, true),
            );
            assert_eq!(draw.receiving_science_hidden(), Some(hide), "{tag}");
            assert_eq!(
                DrawModule::is_visible(&draw),
                !row["hidden"].as_bool().unwrap(),
                "{tag}"
            );
        }
    }

    #[test]
    fn module_data_defaults_required_science_to_invalid() {
        let data = W3DScienceModelDrawModuleData::new();
        assert_eq!(data.required_science, SCIENCE_INVALID);
    }

    #[test]
    fn module_name_key_is_science_model_draw_not_base_model_draw() {
        let draw = W3DScienceModelDraw::new(W3DScienceModelDrawModuleData::new());
        assert_eq!(
            draw.get_module_name_key(),
            NameKeyGenerator::name_to_key("W3DScienceModelDraw")
        );
    }

    #[test]
    fn leftover_invalid_science_hides_like_cpp_do_draw_module() {
        assert!(leftover_science_model_should_hide(SCIENCE_INVALID));
    }

    #[test]
    fn invalid_required_science_hides_like_cpp() {
        assert!(leftover_science_model_should_hide(SCIENCE_INVALID));
        let mut draw = W3DScienceModelDraw::new(W3DScienceModelDrawModuleData::new());
        draw.do_draw_module(
            &Matrix3D::IDENTITY,
            &crate::helpers::DrawTimeObservation::new(None, false, false, true),
        );
        assert!(!draw.is_visible());
    }
}
