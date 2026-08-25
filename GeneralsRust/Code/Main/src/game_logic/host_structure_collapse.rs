//! Host StructureCollapseUpdate residual (buildings sink/collapse on death).
//!
//! C++: `StructureCollapseUpdate::onDie` → delay shudder → sink with gravity
//! damping → POST_COLLAPSE / done.
//!
//! Residual playability slice:
//! - States: Standing → WaitingForStart → Collapsing → Done
//! - Delay frames (default 15–30 @ 30 FPS ≈ 500–1000 ms retail)
//! - Vertical sink offset for presentation (`collapse_height_offset`)
//! - Shudder residual (horizontal noise magnitude, presentation only)
//! - On done: DEATH_TOPPLED + destroy (rubble/post-collapse residual)
//! - INITIAL / DELAY / BURST / FINAL FXList via `doPhaseStuff` (first authored name)
//!
//! Fail-closed:
//! - Not full OCL / bone debris / multi-list GameLogicRandomValue index
//! - FX: first authored list entry per phase
//! - Mid-collapse BURST vs DELAY: BURST only when `BigBurstFrequency == 1`
//! - Not full drawable instance-matrix client shudder
//! - Not full DieMux death-type filters

use serde::{Deserialize, Serialize};
use std::cell::RefCell;

/// C++ COLLAPSE_ACCELERATION uses GlobalData gravity residual.
pub const STRUCTURE_COLLAPSE_GRAVITY: f32 = -1.0;
/// Default collapse damping residual (0 = full gravity).
pub const STRUCTURE_COLLAPSE_DAMPING_DEFAULT: f32 = 0.0;
/// Default max shudder residual (client visual).
pub const STRUCTURE_COLLAPSE_MAX_SHUDDER: f32 = 0.6;
/// Default min/max collapse delay frames (500–1000 ms → 15–30 f).
pub const STRUCTURE_COLLAPSE_DELAY_MIN: u32 = 15;
pub const STRUCTURE_COLLAPSE_DELAY_MAX: u32 = 30;
/// Default geometry height residual when unknown.
pub const STRUCTURE_COLLAPSE_DEFAULT_HEIGHT: f32 = 35.0;
/// C++ default `m_minBurstDelay` when the INI field is omitted.
pub const STRUCTURE_COLLAPSE_MIN_BURST_DELAY_DEFAULT: u32 = 9999;
/// Fail-closed burst-timer residual when leftover min/max are unusable.
pub const STRUCTURE_COLLAPSE_BURST_DELAY_FALLBACK: u32 = 15;

thread_local! {
    static COLLAPSE_INI_OVERRIDE: RefCell<Option<(String, HostStructureCollapseIni)>> =
        const { RefCell::new(None) };
}

/// Authored `StructureCollapseUpdate` Object INI peel (frames, first FX name).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct HostStructureCollapseIni {
    pub min_burst_delay: u32,
    pub max_burst_delay: u32,
    pub big_burst_frequency: i32,
    pub collapse_damping: Option<f32>,
    pub max_shudder: Option<f32>,
    /// C++ `FXList = INITIAL ...` first non-empty FXList name.
    pub fx_initial: Option<String>,
    /// C++ `FXList = DELAY ...` first non-empty FXList name.
    pub fx_delay: Option<String>,
    /// C++ `FXList = BURST ...` first non-empty FXList name.
    pub fx_burst: Option<String>,
    /// C++ `FXList = FINAL ...` first non-empty FXList name.
    pub fx_final: Option<String>,
}

impl HostStructureCollapseIni {
    pub fn has_authored_phase_fx(&self) -> bool {
        self.fx_initial.is_some()
            || self.fx_delay.is_some()
            || self.fx_burst.is_some()
            || self.fx_final.is_some()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum HostStructureCollapseState {
    #[default]
    Standing = 0,
    WaitingForStart = 1,
    Collapsing = 2,
    Done = 3,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostStructureCollapseData {
    pub state: HostStructureCollapseState,
    pub collapse_start_frame: u32,
    pub collapse_velocity: f32,
    /// C++ m_currentHeight (negative as building sinks into ground).
    pub current_height: f32,
    pub collapse_damping: f32,
    pub max_shudder: f32,
    pub building_height: f32,
    /// Presentation lean unused; use height offset + shudder.
    pub shudder_x: f32,
    pub shudder_z: f32,
    /// C++ `m_burstFrame`.
    #[serde(default)]
    pub burst_frame: u32,
    /// C++ `m_minBurstDelay` (logic frames).
    #[serde(default = "default_min_burst_delay")]
    pub min_burst_delay: u32,
    /// C++ `m_maxBurstDelay` (logic frames).
    #[serde(default)]
    pub max_burst_delay: u32,
    /// C++ `m_bigBurstFrequency`.
    #[serde(default)]
    pub big_burst_frequency: i32,
    #[serde(default)]
    pub fx_initial: Option<String>,
    #[serde(default)]
    pub fx_delay: Option<String>,
    #[serde(default)]
    pub fx_burst: Option<String>,
    #[serde(default)]
    pub fx_final: Option<String>,
    #[serde(default)]
    pub pending_phase_fx: Vec<String>,
    #[serde(default)]
    pub initial_played: bool,
    #[serde(default)]
    pub start_burst_played: bool,
    #[serde(default)]
    pub final_played: bool,
}

fn default_min_burst_delay() -> u32 {
    STRUCTURE_COLLAPSE_MIN_BURST_DELAY_DEFAULT
}

impl Default for HostStructureCollapseData {
    fn default() -> Self {
        Self {
            state: HostStructureCollapseState::Standing,
            collapse_start_frame: 0,
            collapse_velocity: 0.0,
            current_height: 0.0,
            collapse_damping: STRUCTURE_COLLAPSE_DAMPING_DEFAULT,
            max_shudder: STRUCTURE_COLLAPSE_MAX_SHUDDER,
            building_height: STRUCTURE_COLLAPSE_DEFAULT_HEIGHT,
            shudder_x: 0.0,
            shudder_z: 0.0,
            burst_frame: 0,
            min_burst_delay: STRUCTURE_COLLAPSE_MIN_BURST_DELAY_DEFAULT,
            max_burst_delay: 0,
            big_burst_frequency: 0,
            fx_initial: None,
            fx_delay: None,
            fx_burst: None,
            fx_final: None,
            pending_phase_fx: Vec::new(),
            initial_played: false,
            start_burst_played: false,
            final_played: false,
        }
    }
}

impl HostStructureCollapseData {
    pub fn is_standing(&self) -> bool {
        self.state == HostStructureCollapseState::Standing
    }

    pub fn is_active(&self) -> bool {
        matches!(
            self.state,
            HostStructureCollapseState::WaitingForStart | HostStructureCollapseState::Collapsing
        )
    }

    /// Presentation vertical offset (negative sinks mesh).
    pub fn collapse_height_offset(&self) -> f32 {
        self.current_height
    }

    pub fn bind_ini(&mut self, ini: &HostStructureCollapseIni) {
        self.min_burst_delay = ini.min_burst_delay;
        self.max_burst_delay = ini.max_burst_delay;
        self.big_burst_frequency = ini.big_burst_frequency;
        if let Some(d) = ini.collapse_damping {
            self.collapse_damping = d;
        }
        if let Some(s) = ini.max_shudder {
            self.max_shudder = s;
        }
        self.fx_initial = ini.fx_initial.clone();
        self.fx_delay = ini.fx_delay.clone();
        self.fx_burst = ini.fx_burst.clone();
        self.fx_final = ini.fx_final.clone();
    }

    /// C++ `doPhaseStuff` — first authored FXList for a phase.
    fn queue_fx(&mut self, name: &Option<String>) {
        if let Some(fx) = name.as_ref().filter(|s| !s.is_empty()) {
            if !fx.eq_ignore_ascii_case("None") {
                self.pending_phase_fx.push(fx.clone());
            }
        }
    }

    fn queue_phase_initial(&mut self) {
        if self.initial_played {
            return;
        }
        self.initial_played = true;
        self.queue_fx(&self.fx_initial.clone());
    }

    fn queue_phase_start_burst(&mut self, current_frame: u32) {
        if self.start_burst_played {
            return;
        }
        self.start_burst_played = true;
        self.queue_fx(&self.fx_burst.clone());
        self.burst_frame = current_frame.saturating_add(self.burst_delay_frames());
    }

    fn queue_phase_final(&mut self) {
        if self.final_played {
            return;
        }
        self.final_played = true;
        self.queue_fx(&self.fx_final.clone());
    }

    /// C++ `GameLogicRandomValue(1, m_bigBurstFrequency) == 1` → BURST else DELAY.
    /// Fail-closed: BURST only when frequency is exactly 1.
    fn queue_mid_collapse_phase(&mut self) {
        if self.big_burst_frequency == 1 {
            self.queue_fx(&self.fx_burst.clone());
        } else {
            self.queue_fx(&self.fx_delay.clone());
        }
    }

    fn burst_delay_frames(&self) -> u32 {
        let min = self.min_burst_delay;
        let max = self.max_burst_delay;
        if min == 0 && max == 0 {
            return STRUCTURE_COLLAPSE_BURST_DELAY_FALLBACK;
        }
        let (lo, hi) = if min <= max { (min, max) } else { (max, min) };
        if hi == 0 {
            return STRUCTURE_COLLAPSE_BURST_DELAY_FALLBACK;
        }
        // Fail-closed midpoint (no GameLogicRandomValue).
        lo.saturating_add(hi.saturating_sub(lo) / 2).max(1)
    }

    fn fire_burst_timer(&mut self, current_frame: u32) {
        if self.burst_frame == 0 || current_frame < self.burst_frame {
            return;
        }
        self.queue_mid_collapse_phase();
        self.burst_frame = self.burst_frame.saturating_add(self.burst_delay_frames());
    }

    /// C++ beginStructureCollapse residual + `doPhaseStuff(SCPHASE_INITIAL)`.
    pub fn begin(&mut self, current_frame: u32, delay_frames: u32) {
        if !self.is_standing() {
            return;
        }
        self.collapse_start_frame = current_frame.saturating_add(delay_frames);
        self.collapse_velocity = 0.0;
        self.current_height = 0.0;
        self.shudder_x = 0.0;
        self.shudder_z = 0.0;
        self.burst_frame = 0;
        self.initial_played = false;
        self.start_burst_played = false;
        self.final_played = false;
        self.pending_phase_fx.clear();
        self.state = HostStructureCollapseState::WaitingForStart;
        self.queue_phase_initial();
    }

    /// Deterministic shudder peel (logic-synced residual, not client RNG).
    fn update_shudder(&mut self, frame: u32) {
        if self.max_shudder <= 0.0 {
            self.shudder_x = 0.0;
            self.shudder_z = 0.0;
            return;
        }
        // Cheap deterministic oscillation residual.
        let t = frame as f32 * 0.37;
        self.shudder_x = (t.sin()) * self.max_shudder;
        self.shudder_z = ((t * 1.3).cos()) * self.max_shudder;
    }

    /// C++ `update` INITIAL already played; BURST at sink start, DELAY/BURST on
    /// timer, FINAL when fully sunk — even when a dual-peel owns sink motion.
    pub fn poll_phase_fx(&mut self, current_frame: u32) {
        if self.state == HostStructureCollapseState::Standing {
            return;
        }
        self.queue_phase_initial();
        if current_frame >= self.collapse_start_frame {
            self.queue_phase_start_burst(current_frame);
        }
        if self.start_burst_played && self.state != HostStructureCollapseState::Done {
            self.fire_burst_timer(current_frame);
        }
        if self.state == HostStructureCollapseState::Done {
            self.queue_phase_final();
        }
    }

    pub fn take_pending_phase_fx(&mut self) -> Vec<String> {
        std::mem::take(&mut self.pending_phase_fx)
    }

    pub fn has_authored_phase_fx(&self) -> bool {
        self.fx_initial.is_some()
            || self.fx_delay.is_some()
            || self.fx_burst.is_some()
            || self.fx_final.is_some()
    }

    /// One logic frame. Returns true when collapse completes.
    pub fn tick(&mut self, current_frame: u32) -> bool {
        let done = match self.state {
            HostStructureCollapseState::Standing | HostStructureCollapseState::Done => false,
            HostStructureCollapseState::WaitingForStart => {
                self.update_shudder(current_frame);
                if current_frame >= self.collapse_start_frame {
                    self.state = HostStructureCollapseState::Collapsing;
                    self.collapse_velocity = 0.0;
                    self.queue_phase_start_burst(current_frame);
                }
                false
            }
            HostStructureCollapseState::Collapsing => {
                // C++: m_currentHeight -= m_collapseVelocity;
                // m_collapseVelocity -= gravity * (1 - damping);
                // Note gravity is negative → velocity becomes more negative → height decreases.
                self.current_height -= self.collapse_velocity;
                self.collapse_velocity -=
                    STRUCTURE_COLLAPSE_GRAVITY * (1.0 - self.collapse_damping);
                self.update_shudder(current_frame);
                self.fire_burst_timer(current_frame);
                // Done when fully below ground: height + buildingHeight <= 0.
                if self.current_height + self.building_height <= 0.0 {
                    self.current_height = -self.building_height;
                    self.shudder_x = 0.0;
                    self.shudder_z = 0.0;
                    self.state = HostStructureCollapseState::Done;
                    self.queue_phase_final();
                    true
                } else {
                    false
                }
            }
        };
        done
    }
}

/// Civilian / prop buildings prefer StructureCollapse over StructureTopple.
pub fn prefers_structure_collapse(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.contains("warfactory")
        || n.contains("barracks")
        || n.contains("commandcenter")
        || n.contains("command_center")
        || n.contains("airfield")
        || n.contains("helipad")
        || n.contains("strategycenter")
        || n.contains("supplycenter")
        || n.contains("powerplant")
        || n.contains("nuclear")
        || n.contains("scud")
        || n.contains("stinger")
        || n.contains("patriot")
        || n.contains("firebase")
        || n.contains("gattling")
        || n.contains("tunnel")
        || n.contains("bunker") && !n.contains("civilian")
    {
        return false; // military → topple residual
    }
    n.contains("civilian")
        || n.contains("barn")
        || n.contains("house")
        || n.contains("hut")
        || n.contains("shack")
        || n.contains("store")
        || n.contains("shop")
        || n.contains("church")
        || n.contains("temple")
        || n.contains("farm")
        || n.contains("stable")
        || n.contains("garage")
        || n.contains("office")
        || n.contains("apartment")
        || n.contains("building")
        || n.contains("tower") && n.contains("water")
        || n.contains("silo")
        || n.contains("warehouse")
        || n.contains("hangar") && n.contains("civ")
}

pub fn is_structure_collapse_candidate(template_name: &str, is_structure: bool) -> bool {
    is_structure && prefers_structure_collapse(template_name)
}

fn first_nonempty_fx_name(raw: &str) -> Option<String> {
    raw.split_whitespace()
        .map(str::trim)
        .find(|tok| {
            !tok.is_empty()
                && !tok.eq_ignore_ascii_case("none")
                && !tok.eq_ignore_ascii_case("initial")
                && !tok.eq_ignore_ascii_case("delay")
                && !tok.eq_ignore_ascii_case("burst")
                && !tok.eq_ignore_ascii_case("final")
        })
        .map(|s| s.to_string())
}

/// Parse `FXList = INITIAL Name` / newline-concatenated multi `FXList =` lines.
pub fn parse_collapse_phase_fx(
    attrs: &[(&str, &str)],
) -> (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
) {
    let mut initial = None;
    let mut delay = None;
    let mut burst = None;
    let mut final_fx = None;
    for (key, value) in attrs {
        if !key.eq_ignore_ascii_case("FXList") {
            continue;
        }
        for line in value.split('\n') {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let mut toks = line.split_whitespace();
            let Some(phase) = toks.next() else {
                continue;
            };
            let rest = toks.collect::<Vec<_>>().join(" ");
            let name = first_nonempty_fx_name(&rest);
            match phase.to_ascii_uppercase().as_str() {
                "INITIAL" => {
                    if initial.is_none() {
                        initial = name;
                    }
                }
                "DELAY" => {
                    if delay.is_none() {
                        delay = name;
                    }
                }
                "BURST" => {
                    if burst.is_none() {
                        burst = name;
                    }
                }
                "FINAL" => {
                    if final_fx.is_none() {
                        final_fx = name;
                    }
                }
                _ => {}
            }
        }
    }
    (initial, delay, burst, final_fx)
}

fn parse_msec_or_frames(raw: &str) -> Option<u32> {
    let token = raw.split_whitespace().next()?;
    let n: u32 = token.parse().ok()?;
    Some(crate::game_logic::host_slow_death::msec_to_logic_frames(n))
}

fn parse_i32_token(raw: &str) -> Option<i32> {
    raw.split_whitespace().next()?.parse().ok()
}

fn parse_real_token(raw: &str) -> Option<f32> {
    raw.split_whitespace().next()?.parse().ok()
}

/// Build authored collapse INI from `Behavior = StructureCollapseUpdate` tokens.
pub fn collapse_ini_from_behavior_attrs(attrs: &[(&str, &str)]) -> HostStructureCollapseIni {
    let get = |key: &str| {
        attrs
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(key))
            .map(|(_, v)| *v)
    };
    let (fx_initial, fx_delay, fx_burst, fx_final) = parse_collapse_phase_fx(attrs);
    HostStructureCollapseIni {
        min_burst_delay: get("MinBurstDelay")
            .and_then(parse_msec_or_frames)
            .unwrap_or(STRUCTURE_COLLAPSE_MIN_BURST_DELAY_DEFAULT),
        max_burst_delay: get("MaxBurstDelay")
            .and_then(parse_msec_or_frames)
            .unwrap_or(0),
        big_burst_frequency: get("BigBurstFrequency")
            .and_then(parse_i32_token)
            .unwrap_or(0),
        collapse_damping: get("CollapseDamping").and_then(parse_real_token),
        max_shudder: get("MaxShudder").and_then(parse_real_token),
        fx_initial,
        fx_delay,
        fx_burst,
        fx_final,
    }
}

fn first_named_fx_slot<T>(list: &[Option<std::sync::Arc<T>>]) -> Option<String>
where
    T: FxNamed,
{
    for slot in list {
        if let Some(fx) = slot {
            let n = fx.fx_name().trim();
            if !n.is_empty() && !n.eq_ignore_ascii_case("None") {
                return Some(n.to_string());
            }
        }
    }
    None
}

trait FxNamed {
    fn fx_name(&self) -> &str;
}

impl FxNamed for gamelogic::effects::FXList {
    fn fx_name(&self) -> &str {
        self.name()
    }
}

fn leftover_structure_collapse_module_peel(
    template_name: &str,
) -> Option<HostStructureCollapseIni> {
    let guard = game_engine::common::thing::thing_factory::try_get_thing_factory()?;
    let factory = guard.as_ref()?;
    let tmpl = factory.find_template(template_name, false)?;
    for entry in tmpl.get_behavior_module_info().iter() {
        if !entry
            .name
            .as_str()
            .eq_ignore_ascii_case("StructureCollapseUpdate")
        {
            continue;
        }
        if let Some(data) = entry
            .data
            .downcast_ref::<gamelogic::object::behavior::StructureCollapseUpdateModuleData>()
        {
            return Some(HostStructureCollapseIni {
                min_burst_delay: data.min_burst_delay,
                max_burst_delay: data.max_burst_delay,
                big_burst_frequency: data.big_burst_frequency,
                collapse_damping: Some(data.collapse_damping),
                max_shudder: Some(data.max_shudder),
                fx_initial: first_named_fx_slot(&data.fxs[0]),
                fx_delay: first_named_fx_slot(&data.fxs[1]),
                fx_burst: first_named_fx_slot(&data.fxs[2]),
                fx_final: first_named_fx_slot(&data.fxs[3]),
            });
        }
        let fxlist = entry.data.get_ini_field("FXList").unwrap_or("");
        let body = entry
            .data
            .downcast_ref::<game_engine::common::thing::module::CapturedModuleData>()
            .map(|c| c.raw_body())
            .unwrap_or("");
        let combined = if body.is_empty() {
            fxlist.to_string()
        } else {
            format!("{fxlist}\n{body}")
        };
        let mut attrs: Vec<(&str, &str)> = vec![("FXList", combined.as_str())];
        let min_b = entry.data.get_ini_field("MinBurstDelay").unwrap_or("");
        let max_b = entry.data.get_ini_field("MaxBurstDelay").unwrap_or("");
        let freq = entry.data.get_ini_field("BigBurstFrequency").unwrap_or("");
        let damp = entry.data.get_ini_field("CollapseDamping").unwrap_or("");
        let shud = entry.data.get_ini_field("MaxShudder").unwrap_or("");
        if !min_b.is_empty() {
            attrs.push(("MinBurstDelay", min_b));
        }
        if !max_b.is_empty() {
            attrs.push(("MaxBurstDelay", max_b));
        }
        if !freq.is_empty() {
            attrs.push(("BigBurstFrequency", freq));
        }
        if !damp.is_empty() {
            attrs.push(("CollapseDamping", damp));
        }
        if !shud.is_empty() {
            attrs.push(("MaxShudder", shud));
        }
        return Some(collapse_ini_from_behavior_attrs(&attrs));
    }
    None
}

fn asset_manager_collapse_ini(name: &str) -> Option<HostStructureCollapseIni> {
    let manager = crate::assets::get_asset_manager()?;
    let manager = manager.lock().ok()?;
    let definition = manager.get_object_definition(name)?;
    for module in &definition.behavior_modules {
        if !module
            .class_name
            .eq_ignore_ascii_case("StructureCollapseUpdate")
        {
            continue;
        }
        let attrs: Vec<(&str, &str)> = module
            .attributes
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        return Some(collapse_ini_from_behavior_attrs(&attrs));
    }
    None
}

fn authored_collapse_ini(name: &str) -> Option<HostStructureCollapseIni> {
    let leftover = leftover_structure_collapse_module_peel(name);
    let assets = asset_manager_collapse_ini(name);
    match (leftover, assets) {
        (Some(mut ini), Some(am)) => {
            if !ini.has_authored_phase_fx() {
                ini.fx_initial = am.fx_initial;
                ini.fx_delay = am.fx_delay;
                ini.fx_burst = am.fx_burst;
                ini.fx_final = am.fx_final;
            }
            Some(ini)
        }
        (Some(ini), None) => Some(ini),
        (None, Some(am)) => Some(am),
        (None, None) => None,
    }
}

/// Test helper: treat `template_name` as authoring StructureCollapseUpdate FX.
pub fn override_collapse_ini_for_tests(template_name: &str, ini: HostStructureCollapseIni) {
    COLLAPSE_INI_OVERRIDE.with(|slot| {
        *slot.borrow_mut() = Some((template_name.to_string(), ini));
    });
}

pub fn clear_collapse_ini_override_for_tests() {
    COLLAPSE_INI_OVERRIDE.with(|slot| {
        *slot.borrow_mut() = None;
    });
}

/// Live lookup: override (tests) then leftover ThingFactory then Object INI.
pub fn collapse_ini_for_template(name: &str) -> Option<HostStructureCollapseIni> {
    let hit = COLLAPSE_INI_OVERRIDE.with(|slot| {
        slot.borrow()
            .as_ref()
            .and_then(|(n, ini)| n.eq_ignore_ascii_case(name).then(|| ini.clone()))
    });
    if hit.is_some() {
        return hit;
    }
    authored_collapse_ini(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structure_collapse_sinks_and_completes() {
        let mut c = HostStructureCollapseData::default();
        c.building_height = 20.0;
        c.begin(0, 0);
        assert_eq!(c.state, HostStructureCollapseState::WaitingForStart);
        let mut done = false;
        for f in 0..600 {
            if c.tick(f) {
                done = true;
                break;
            }
        }
        assert!(done);
        assert_eq!(c.state, HostStructureCollapseState::Done);
        assert!(c.collapse_height_offset() <= -20.0 + 1e-3);
    }

    #[test]
    fn civilian_prefers_collapse() {
        assert!(prefers_structure_collapse("CivilianBarn01"));
        assert!(!prefers_structure_collapse("AmericaWarFactory"));
    }

    #[test]
    fn parse_phase_fx_initial_delay_burst_final() {
        let ini = collapse_ini_from_behavior_attrs(&[
            ("MinBurstDelay", "250"),
            ("MaxBurstDelay", "800"),
            ("BigBurstFrequency", "4"),
            (
                "FXList",
                "INITIAL FX_StructureMediumCollapse\nDELAY FX_StructureCollapseDelay\nBURST FX_StructureCollapseBurst\nFINAL FX_StructureCollapseFinal",
            ),
        ]);
        assert_eq!(
            ini.fx_initial.as_deref(),
            Some("FX_StructureMediumCollapse")
        );
        assert_eq!(ini.fx_delay.as_deref(), Some("FX_StructureCollapseDelay"));
        assert_eq!(ini.fx_burst.as_deref(), Some("FX_StructureCollapseBurst"));
        assert_eq!(ini.fx_final.as_deref(), Some("FX_StructureCollapseFinal"));
        assert_eq!(
            ini.min_burst_delay,
            crate::game_logic::host_slow_death::msec_to_logic_frames(250)
        );
    }

    #[test]
    fn phase_fx_initial_burst_delay_final() {
        let ini = collapse_ini_from_behavior_attrs(&[
            ("MinBurstDelay", "66"),
            ("MaxBurstDelay", "66"),
            ("BigBurstFrequency", "4"),
            (
                "FXList",
                "INITIAL FX_DieInitial\nDELAY FX_DieDelay\nBURST FX_DieBurst\nFINAL FX_DieFinal",
            ),
        ]);
        let mut d = HostStructureCollapseData::default();
        d.building_height = 4.0;
        d.bind_ini(&ini);
        d.begin(0, 0);
        assert_eq!(d.take_pending_phase_fx(), vec!["FX_DieInitial"]);
        // Frame 0 still WaitingForStart until tick sees start frame.
        assert!(!d.tick(0));
        assert_eq!(d.state, HostStructureCollapseState::Collapsing);
        assert_eq!(d.take_pending_phase_fx(), vec!["FX_DieBurst"]);
        // 66ms → 2 frames; midpoint delay is 2.
        let burst_at = d.burst_frame;
        assert!(burst_at >= 2);
        assert!(!d.tick(burst_at));
        assert_eq!(d.take_pending_phase_fx(), vec!["FX_DieDelay"]);
        let mut done = false;
        for f in burst_at + 1..200 {
            if d.tick(f) {
                done = true;
                break;
            }
        }
        assert!(done);
        assert_eq!(d.take_pending_phase_fx(), vec!["FX_DieFinal"]);
    }

    #[test]
    fn poll_phase_fx_does_not_double_fire_after_tick() {
        let ini = collapse_ini_from_behavior_attrs(&[(
            "FXList",
            "INITIAL FX_DieInitial\nBURST FX_DieBurst\nFINAL FX_DieFinal",
        )]);
        let mut d = HostStructureCollapseData::default();
        d.bind_ini(&ini);
        d.begin(0, 5);
        assert_eq!(d.take_pending_phase_fx(), vec!["FX_DieInitial"]);
        d.poll_phase_fx(0);
        assert!(d.take_pending_phase_fx().is_empty());
        assert!(!d.tick(5));
        assert_eq!(d.take_pending_phase_fx(), vec!["FX_DieBurst"]);
        d.poll_phase_fx(5);
        assert!(d.take_pending_phase_fx().is_empty());
    }

    #[test]
    fn live_host_dispatches_phase_fx_on_death_path() {
        let death = include_str!("object/death.rs");
        assert!(
            death.contains("dispatch_pending_collapse_fx"),
            "live begin/tick must drain collapse phase FX"
        );
        assert!(
            death.contains("dispatch_fx_list_at_pos"),
            "C++ FXList::doFXPos must run on the live collapse path"
        );
        assert!(
            death.contains("poll_structure_collapse_phase_fx"),
            "dual-peel must still play BURST/DELAY/FINAL"
        );
        let tick = include_str!("world_tick/ai.rs");
        assert!(
            tick.contains("poll_structure_collapse_phase_fx"),
            "world tick must poll collapse FX when GW owns the sink"
        );
    }
}
