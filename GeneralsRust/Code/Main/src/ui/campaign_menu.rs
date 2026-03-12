//! Campaign Mission Select Screen
//!
//! This module implements the campaign selection and mission browser
//! matching the C&C Generals campaign structure.

use super::{
    layout, utils, Interactive, KeyCode, MouseButton, Renderable, Screen, UIEvent, UIRenderContext,
};
use crate::localization;
use log::info;

/// Campaign factions (from C++ campaign system)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CampaignFaction {
    USA,
    China,
    GLA,
}

/// Mission in a campaign
#[derive(Debug, Clone)]
pub struct Mission {
    pub id: String,
    pub name: String,
    pub description: String,
    pub map_name: String,
    pub unlocked: bool,
    pub completed: bool,
    pub medal: Option<MedalType>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MedalType {
    Bronze,
    Silver,
    Gold,
}

/// Campaign Menu implementation
pub struct CampaignMenu {
    /// Selected faction
    selected_faction: Option<CampaignFaction>,
    /// List of missions for selected faction
    missions: Vec<Mission>,
    /// Currently selected mission
    selected_mission: Option<usize>,
    /// Screen dimensions
    screen_size: (u32, u32),
    /// Animation progress
    animation_progress: f32,
}

impl Default for CampaignMenu {
    fn default() -> Self {
        Self::new()
    }
}

impl CampaignMenu {
    fn text(key: &str, fallback: &str) -> String {
        localization::localize(key, fallback)
    }

    pub fn new() -> Self {
        Self {
            selected_faction: None,
            missions: Vec::new(),
            selected_mission: None,
            screen_size: (1024, 768),
            animation_progress: 0.0,
        }
    }

    pub fn initialize(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Show faction selection initially
        self.selected_faction = None;
        Ok(())
    }

    pub fn update(&mut self, delta_time: f32) -> Result<(), Box<dyn std::error::Error>> {
        if self.animation_progress < 1.0 {
            self.animation_progress += delta_time * 2.0;
            self.animation_progress = self.animation_progress.min(1.0);
        }
        Ok(())
    }

    pub fn select_faction(&mut self, faction: CampaignFaction) {
        self.selected_faction = Some(faction);
        self.load_faction_missions(faction);
    }

    fn load_faction_missions(&mut self, faction: CampaignFaction) {
        self.missions.clear();

        // Load missions for the selected faction
        match faction {
            CampaignFaction::USA => {
                self.missions.push(Mission {
                    id: "USA01".to_string(),
                    name: Self::text("campaign.usa_01", "First Strike"),
                    description: Self::text("campaign.usa_01_desc", "Baghdad, Iraq"),
                    map_name: "USA01_FirstStrike".to_string(),
                    unlocked: true,
                    completed: false,
                    medal: None,
                });
                self.missions.push(Mission {
                    id: "USA02".to_string(),
                    name: Self::text("campaign.usa_02", "The Second Front"),
                    description: Self::text("campaign.usa_02_desc", "Kazakhstan"),
                    map_name: "USA02_SecondFront".to_string(),
                    unlocked: false,
                    completed: false,
                    medal: None,
                });
            }
            CampaignFaction::China => {
                self.missions.push(Mission {
                    id: "CHI01".to_string(),
                    name: Self::text("campaign.china_01", "Black Lotus"),
                    description: Self::text("campaign.china_01_desc", "Hong Kong"),
                    map_name: "CHI01_BlackLotus".to_string(),
                    unlocked: true,
                    completed: false,
                    medal: None,
                });
            }
            CampaignFaction::GLA => {
                self.missions.push(Mission {
                    id: "GLA01".to_string(),
                    name: Self::text("campaign.gla_01", "The Call to Arms"),
                    description: Self::text("campaign.gla_01_desc", "Middle East"),
                    map_name: "GLA01_CallToArms".to_string(),
                    unlocked: true,
                    completed: false,
                    medal: None,
                });
            }
        }

        info!(
            "{}",
            localization::localize_with_args(
                "campaign.log.loaded_missions",
                "Loaded {count} missions for {faction:?} campaign",
                &[
                    ("count", &self.missions.len().to_string()),
                    ("faction", &format!("{:?}", faction))
                ],
            )
        );
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.screen_size = (width, height);
    }
}

impl Interactive for CampaignMenu {
    fn handle_mouse_move(&mut self, _x: i32, _y: i32) -> bool {
        false
    }

    fn handle_mouse_click(&mut self, _x: i32, _y: i32, _button: MouseButton) -> bool {
        false
    }

    fn handle_key_press(&mut self, key: KeyCode) -> bool {
        match key {
            KeyCode::Escape => true,
            _ => false,
        }
    }

    fn handle_text_input(&mut self, _text: &str) -> bool {
        false
    }
}

impl Renderable for CampaignMenu {
    fn render(&self, _context: &mut UIRenderContext) {
        println!("{}", Self::text("campaign.log.header", "=== CAMPAIGN ==="));

        if let Some(faction) = self.selected_faction {
            println!(
                "{} {:?}",
                Self::text("campaign.selected_faction", "Faction:"),
                faction
            );

            println!("\n{}", Self::text("campaign.missions_header", "Missions:"));
            for (i, mission) in self.missions.iter().enumerate() {
                let status = if mission.completed {
                    "[COMPLETED]"
                } else if mission.unlocked {
                    "[AVAILABLE]"
                } else {
                    "[LOCKED]"
                };

                let selected_marker = if Some(i) == self.selected_mission {
                    " <--"
                } else {
                    ""
                };

                println!(
                    "  {}. {} {}{}",
                    i + 1,
                    mission.name,
                    status,
                    selected_marker
                );
                println!("     {}", mission.description);

                if let Some(medal) = mission.medal {
                    println!("     Medal: {:?}", medal);
                }
            }
        } else {
            println!(
                "\n{}",
                Self::text("campaign.select_faction", "Select a faction:")
            );
            println!("  1. USA");
            println!("  2. China");
            println!("  3. GLA");
        }
    }

    fn get_bounds(&self) -> (i32, i32, u32, u32) {
        (0, 0, self.screen_size.0, self.screen_size.1)
    }

    fn is_visible(&self) -> bool {
        true
    }
}
