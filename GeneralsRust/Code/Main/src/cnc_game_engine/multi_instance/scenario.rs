//! What one instance plays: an e2e scenario expressed as runtime-host
//! commands, the same control surface `golden_capture` and the gpui gates use.

use super::instance::{InstancePhase, InstanceStatus, Verdict};
use crate::executable_smoke::SmokeGate;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Retail map of the golden skirmish (`golden_skirmish.rs`).
pub const GOLDEN_MAP: &str = "Lone Eagle";

/// One instance's e2e scenario.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstanceScenario {
    /// Stay in the shell: ShellMapMD behind the main menu.
    Shell,
    /// Start a skirmish from the menu (`start_game|mode=skirmish|map=…`).
    Skirmish {
        map: String,
        faction: Option<String>,
    },
    /// Run raw runtime-host commands once the menu is up, in order.
    Commands(Vec<String>),
    /// One retail executable gate (`*_gate` binaries), its smoke driver
    /// attached to this instance's runtime-host bridge. `map` overrides the
    /// gate's Lone Eagle for the skirmish gates.
    Gate {
        gate: SmokeGate,
        map: Option<String>,
    },
}

/// When a host-assessed scenario passes or fails.
#[derive(Debug, Clone, Copy)]
pub struct Assertions {
    /// A skirmish passes once its match is InGame at this logic frame.
    pub pass_logic_frame: u32,
    /// The shell passes after this many presented menu frames.
    pub pass_menu_presents: u64,
    /// Undecided scenarios fail after this long.
    pub timeout: Duration,
}

impl Default for Assertions {
    fn default() -> Self {
        Self {
            // 30 s of game time (C++ LOGICFRAMES_PER_SECOND = 30).
            pass_logic_frame: 900,
            pass_menu_presents: 30,
            timeout: Duration::from_secs(900),
        }
    }
}

impl InstanceScenario {
    /// The golden retail skirmish: Lone Eagle, human vs skirmish AI.
    pub fn golden() -> Self {
        Self::Skirmish {
            map: GOLDEN_MAP.to_string(),
            faction: None,
        }
    }

    /// Short label for tiles and reports.
    pub fn label(&self) -> String {
        match self {
            Self::Shell => "shell".to_string(),
            Self::Skirmish { map, faction: None } => format!("skirmish:{map}"),
            Self::Skirmish {
                map,
                faction: Some(f),
            } => format!("skirmish:{map}:{f}"),
            Self::Commands(cmds) => format!("cmds:{}", cmds.first().map_or("", |c| c.as_str())),
            Self::Gate { gate, map: None } => format!("gate:{}", gate.name()),
            Self::Gate {
                gate,
                map: Some(map),
            } => format!("gate:{}:{map}", gate.name()),
        }
    }

    /// Map shown on the tile label.
    pub fn map_name(&self) -> &str {
        match self {
            Self::Shell => "ShellMapMD",
            Self::Skirmish { map, .. } => map,
            Self::Commands(_) => "-",
            Self::Gate { gate, .. } if !gate.starts_skirmish() => "ShellMapMD",
            Self::Gate { map: Some(map), .. } => map,
            Self::Gate { map: None, .. } => GOLDEN_MAP,
        }
    }

    /// The retail gate of a gate scenario.
    pub fn gate(&self) -> Option<SmokeGate> {
        match self {
            Self::Gate { gate, .. } => Some(*gate),
            _ => None,
        }
    }

    /// Installed map file of a gate scenario's override map.
    pub fn gate_map_path(&self, assets: Option<&Path>) -> Option<String> {
        match self {
            Self::Gate { map: Some(map), .. } => Some(
                resolve_map(map, assets)
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| map.clone()),
            ),
            _ => None,
        }
    }

    /// Host-side verdict of a scenario without an attached driver: its
    /// assertions against the live status. Gate scenarios are judged by
    /// their driver and stay pending here.
    pub(super) fn assess(
        &self,
        status: &InstanceStatus,
        elapsed: Duration,
        assertions: &Assertions,
    ) -> Verdict {
        if let InstancePhase::Failed(reason) = &status.phase {
            return Verdict::Fail(reason.chars().take(80).collect());
        }
        let passed = match self {
            Self::Gate { .. } => return Verdict::Pending,
            Self::Shell => {
                status.state == "Menu" && status.rendered_frames >= assertions.pass_menu_presents
            }
            Self::Skirmish { .. } => {
                status.state == "InGame" && status.logic_frame >= assertions.pass_logic_frame
            }
            Self::Commands(cmds) => {
                status.commands.len() == cmds.len()
                    && status.rendered_frames >= assertions.pass_menu_presents
            }
        };
        if passed {
            return Verdict::Pass(match self {
                Self::Skirmish { .. } => format!("InGame f{}", status.logic_frame),
                _ => format!("{} {} presents", status.state, status.rendered_frames),
            });
        }
        if matches!(status.phase, InstancePhase::Finished) {
            return Verdict::Fail(format!("exited in {} f{}", status.state, status.logic_frame));
        }
        if elapsed >= assertions.timeout {
            return Verdict::Fail(format!(
                "timeout {}s in {} f{}",
                assertions.timeout.as_secs(),
                status.state,
                status.logic_frame
            ));
        }
        Verdict::Pending
    }

    /// Runtime-host commands to apply once the engine reaches the menu.
    pub fn menu_commands(&self, assets: Option<&Path>) -> Vec<String> {
        match self {
            Self::Shell => Vec::new(),
            Self::Skirmish { map, faction } => {
                let map = resolve_map(map, assets)
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| map.clone());
                let mut cmd = format!("start_game|mode=skirmish|map={map}");
                if let Some(f) = faction {
                    cmd.push_str(&format!("|faction={f}"));
                }
                vec![cmd]
            }
            Self::Commands(cmds) => cmds.clone(),
            // The attached driver issues the gate's commands.
            Self::Gate { .. } => Vec::new(),
        }
    }

    /// Whether this instance is ready for the screenshot dump.
    pub(super) fn shot_ready(&self, status: &InstanceStatus, logic_frame: u32) -> bool {
        match status.phase {
            InstancePhase::Failed(_) | InstancePhase::Finished => true,
            InstancePhase::Booting => false,
            InstancePhase::Running => match self {
                Self::Shell => status.state == "Menu" && status.rendered_frames >= 30,
                Self::Gate { .. } => status.verdict.is_decided(),
                _ => status.state == "InGame" && status.logic_frame >= logic_frame,
            },
        }
    }
}

impl std::str::FromStr for InstanceScenario {
    type Err = String;

    /// `shell`, `golden`, `skirmish:<map>[:<faction>]`, or
    /// `cmds:<cmd>;<cmd>;…` (runtime-host command lines), or
    /// `gate:<menu|map_load|orders|ai|save_load>[:<map>]`.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let text = text.trim();
        let (kind, rest) = text.split_once(':').unwrap_or((text, ""));
        match kind.to_ascii_lowercase().as_str() {
            "shell" | "menu" => Ok(Self::Shell),
            "golden" => Ok(Self::golden()),
            "skirmish" => {
                let mut parts = rest.splitn(2, ':');
                let map = parts.next().unwrap_or("").trim();
                let map = if map.is_empty() { GOLDEN_MAP } else { map };
                let faction = parts.next().map(str::trim).filter(|f| !f.is_empty());
                Ok(Self::Skirmish {
                    map: map.to_string(),
                    faction: faction.map(str::to_string),
                })
            }
            "cmds" | "commands" => {
                let cmds: Vec<String> = rest
                    .split(';')
                    .map(str::trim)
                    .filter(|c| !c.is_empty())
                    .map(str::to_string)
                    .collect();
                if cmds.is_empty() {
                    Err(format!("scenario {text:?} has no commands"))
                } else {
                    Ok(Self::Commands(cmds))
                }
            }
            "gate" => {
                let (gate, map) = rest.split_once(':').unwrap_or((rest, ""));
                let map = map.trim();
                Ok(Self::Gate {
                    gate: gate.parse()?,
                    map: (!map.is_empty()).then(|| map.to_string()),
                })
            }
            _ => Err(format!(
                "unknown scenario {text:?} (shell | golden | skirmish:<map>[:<faction>] | cmds:<a>;<b> | gate:<name>[:<map>])"
            )),
        }
    }
}

/// The e2e suite grid: every retail executable gate, the golden skirmish,
/// the shell, and skirmishes across other maps and factions, cycled to `n`
/// tiles so 8 and 16 tiles each show the whole suite.
pub fn e2e_suite(n: usize) -> Vec<InstanceScenario> {
    let gate = |gate: SmokeGate, map: Option<&str>| InstanceScenario::Gate {
        gate,
        map: map.map(str::to_string),
    };
    let skirmish = |map: &str, faction: &str| InstanceScenario::Skirmish {
        map: map.to_string(),
        faction: Some(faction.to_string()),
    };
    let base = [
        gate(SmokeGate::BootShellMenu, None),
        gate(SmokeGate::LoadRetailMap, None),
        gate(SmokeGate::IssueOrders, None),
        gate(SmokeGate::AiOpponent, None),
        gate(SmokeGate::SaveLoad, None),
        InstanceScenario::golden(),
        skirmish("Tournament Desert", "China"),
        InstanceScenario::Shell,
        gate(SmokeGate::LoadRetailMap, Some("Tournament Desert")),
        skirmish("Tournament Island", "GLA"),
        gate(SmokeGate::IssueOrders, Some("Tournament Island")),
        skirmish("Lone Eagle", "America"),
        gate(SmokeGate::LoadRetailMap, Some("Tournament Island")),
        skirmish("Tournament Desert", "GLA"),
        gate(SmokeGate::AiOpponent, Some("Tournament Desert")),
        skirmish("Tournament Island", "China"),
    ];
    base.iter().cloned().cycle().take(n.max(1)).collect()
}

/// Installed map file for a bare map name, in the order `golden_capture`
/// looks: the retail install, then the extracted MapsZH archives, then
/// `GENERALS_MAPS_DIR` (a directory holding `Maps/<name>/<name>.map`).
fn resolve_map(map: &str, assets: Option<&Path>) -> Option<PathBuf> {
    let direct = Path::new(map);
    if direct
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("map"))
    {
        return direct.is_file().then(|| direct.to_path_buf());
    }
    let relative = Path::new("Maps").join(map).join(format!("{map}.map"));
    let extra = std::env::var_os("GENERALS_MAPS_DIR").map(|dir| PathBuf::from(dir).join(&relative));
    let Some(assets) = assets else {
        return extra.filter(|p| p.is_file());
    };
    let mut candidates = vec![assets.join(&relative)];
    candidates.extend(extra);
    if let Some(parent) = assets.parent() {
        for extract in ["extracted_big_files", "extracted_big_files_v2"] {
            candidates.push(parent.join(extract).join("MapsZH").join(&relative));
        }
    }
    candidates.into_iter().find(|p| p.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_scenarios() {
        assert_eq!(
            "shell".parse::<InstanceScenario>().unwrap(),
            InstanceScenario::Shell
        );
        assert_eq!(
            "golden".parse::<InstanceScenario>().unwrap(),
            InstanceScenario::golden()
        );
        assert_eq!(
            "skirmish:Tournament Desert:China"
                .parse::<InstanceScenario>()
                .unwrap(),
            InstanceScenario::Skirmish {
                map: "Tournament Desert".into(),
                faction: Some("China".into())
            }
        );
        assert_eq!(
            "cmds:open_options; menu"
                .parse::<InstanceScenario>()
                .unwrap(),
            InstanceScenario::Commands(vec!["open_options".into(), "menu".into()])
        );
        assert!("bogus".parse::<InstanceScenario>().is_err());
        assert_eq!(
            "gate:orders".parse::<InstanceScenario>().unwrap(),
            InstanceScenario::Gate {
                gate: SmokeGate::IssueOrders,
                map: None
            }
        );
        assert_eq!(
            "gate:map_load:Tournament Desert"
                .parse::<InstanceScenario>()
                .unwrap(),
            InstanceScenario::Gate {
                gate: SmokeGate::LoadRetailMap,
                map: Some("Tournament Desert".into())
            }
        );
        assert!("gate:bogus".parse::<InstanceScenario>().is_err());
    }

    #[test]
    fn labels_round_trip_through_the_parser() {
        for scenario in e2e_suite(16) {
            assert_eq!(scenario.label().parse::<InstanceScenario>().unwrap(), scenario);
        }
    }

    #[test]
    fn e2e_suite_covers_every_gate_in_8_tiles() {
        let suite = e2e_suite(8);
        assert_eq!(suite.len(), 8);
        for gate in SmokeGate::ALL {
            assert!(suite.iter().any(|s| s.gate() == Some(gate)), "{gate:?}");
        }
        assert!(suite.contains(&InstanceScenario::golden()));
        assert_eq!(e2e_suite(16).len(), 16);
    }

    fn status(phase: InstancePhase, state: &str, logic_frame: u32, presents: u64) -> InstanceStatus {
        let mut s = InstanceStatus::new(0, &InstanceScenario::Shell);
        s.phase = phase;
        s.state = state.into();
        s.logic_frame = logic_frame;
        s.rendered_frames = presents;
        s
    }

    #[test]
    fn host_assertions_decide_pass_fail_and_timeout() {
        let a = Assertions::default();
        let t = Duration::from_secs(1);
        let golden = InstanceScenario::golden();
        assert_eq!(
            golden.assess(&status(InstancePhase::Running, "InGame", 10, 5), t, &a),
            Verdict::Pending
        );
        assert!(matches!(
            golden.assess(&status(InstancePhase::Running, "InGame", 900, 5), t, &a),
            Verdict::Pass(_)
        ));
        assert!(matches!(
            golden.assess(&status(InstancePhase::Failed("boom".into()), "InGame", 900, 5), t, &a),
            Verdict::Fail(_)
        ));
        assert!(matches!(
            golden.assess(&status(InstancePhase::Running, "Loading", 0, 0), a.timeout, &a),
            Verdict::Fail(_)
        ));
        assert!(matches!(
            InstanceScenario::Shell.assess(&status(InstancePhase::Running, "Menu", 0, 30), t, &a),
            Verdict::Pass(_)
        ));
        let gate = InstanceScenario::Gate {
            gate: SmokeGate::BootShellMenu,
            map: None,
        };
        assert_eq!(
            gate.assess(&status(InstancePhase::Running, "Menu", 0, 300), a.timeout, &a),
            Verdict::Pending
        );
    }

    #[test]
    fn skirmish_command_falls_back_to_the_map_name() {
        let empty = tempfile::tempdir().unwrap();
        let map = empty.path().join("Uninstalled Fixture.map").display().to_string();
        let scenario = InstanceScenario::Skirmish {
            map: map.clone(),
            faction: None,
        };
        assert_eq!(scenario.menu_commands(None), vec![format!("start_game|mode=skirmish|map={map}")]);
    }

    #[test]
    fn grid_is_4x2_for_8_and_4x4_for_16() {
        assert_eq!(super::super::grid_columns(8), 4);
        assert_eq!(super::super::grid_columns(16), 4);
        assert_eq!(super::super::grid_columns(4), 2);
    }
}
