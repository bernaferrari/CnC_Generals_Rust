//! What one instance plays: an e2e scenario expressed as runtime-host
//! commands, the same control surface `golden_capture` and the gpui gates use.

use super::instance::{InstancePhase, InstanceStatus};
use std::path::{Path, PathBuf};

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
        }
    }

    /// Map shown on the tile label.
    pub fn map_name(&self) -> &str {
        match self {
            Self::Shell => "ShellMapMD",
            Self::Skirmish { map, .. } => map,
            Self::Commands(_) => "-",
        }
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
        }
    }

    /// Whether this instance is ready for the screenshot dump.
    pub(super) fn shot_ready(&self, status: &InstanceStatus, logic_frame: u32) -> bool {
        match status.phase {
            InstancePhase::Failed(_) | InstancePhase::Finished => true,
            InstancePhase::Booting => false,
            InstancePhase::Running => match self {
                Self::Shell => status.state == "Menu" && status.rendered_frames >= 30,
                _ => status.state == "InGame" && status.logic_frame >= logic_frame,
            },
        }
    }
}

impl std::str::FromStr for InstanceScenario {
    type Err = String;

    /// `shell`, `golden`, `skirmish:<map>[:<faction>]`, or
    /// `cmds:<cmd>;<cmd>;…` (runtime-host command lines).
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
            _ => Err(format!(
                "unknown scenario {text:?} (shell | golden | skirmish:<map>[:<faction>] | cmds:<a>;<b>)"
            )),
        }
    }
}

/// Installed map file for a bare map name, in the order `golden_capture`
/// looks: the retail install, then the extracted MapsZH archives.
fn resolve_map(map: &str, assets: Option<&Path>) -> Option<PathBuf> {
    let direct = Path::new(map);
    if direct
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("map"))
    {
        return direct.is_file().then(|| direct.to_path_buf());
    }
    let assets = assets?;
    let relative = Path::new("Maps").join(map).join(format!("{map}.map"));
    let mut candidates = vec![assets.join(&relative)];
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
    }

    #[test]
    fn skirmish_command_falls_back_to_the_map_name() {
        let cmds = InstanceScenario::golden().menu_commands(None);
        assert_eq!(
            cmds,
            vec!["start_game|mode=skirmish|map=Lone Eagle".to_string()]
        );
    }

    #[test]
    fn grid_is_4x2_for_8_and_4x4_for_16() {
        assert_eq!(super::super::grid_columns(8), 4);
        assert_eq!(super::super::grid_columns(16), 4);
        assert_eq!(super::super::grid_columns(4), 2);
    }
}
