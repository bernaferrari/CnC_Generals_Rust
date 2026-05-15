// Basic type aliases common across RTS modules
pub type Real = f32;
pub type AsciiString = String;
pub type UnicodeString = String;
pub type NameKeyType = u32;
pub type UnsignedShort = u16;
pub type UnsignedByte = u8;
/// Color constants and utilities
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color(pub u32);

impl Color {
    pub const fn white() -> Self {
        Color(0xFFFFFFFF)
    }

    pub const fn black() -> Self {
        Color(0xFF000000)
    }

    pub const fn red() -> Self {
        Color(0xFFFF0000)
    }

    pub const fn green() -> Self {
        Color(0xFF00FF00)
    }

    pub const fn blue() -> Self {
        Color(0xFF0000FF)
    }

    pub const fn transparent() -> Self {
        Color(0x00000000)
    }
}

// Real-time strategy game modules
pub mod academy_stats;
pub mod achievements;
pub mod action_manager;
pub mod energy;
pub mod handicap;
pub mod handles;
pub mod mission_stats;
pub mod money;
pub mod multiplayer_rankings;
pub mod player;
pub mod player_list;
pub mod player_template;
pub mod post_game_stats;
pub mod production_prerequisite;
pub mod resource_gathering_manager;
pub mod science;
pub mod score_keeper;
pub mod special_power;
pub mod team;
pub mod tunnel_tracker;

// Re-export main types for easier access
pub use academy_stats::{AcademyAdviceInfo, AcademyClassificationType, AcademyStats};
pub use achievements::{Achievement, AchievementCalculator, AchievementType};
pub use action_manager::{
    set_object_data_provider, ActionManager, Object, ObjectDataProvider, WeaponSlotType,
};
pub use energy::Energy;
pub use handicap::{Handicap, HandicapType, ThingType};
pub use handles::{
    CommandSetHandle, FrameNumber, ObjectHandle, PlayerHandle, SpecialPowerHandle,
    ThingTemplateHandle, UpgradeHandle,
};
pub use mission_stats::MissionStats;
pub use money::Money;
pub use multiplayer_rankings::{ELOCalculator, GameMode, PlayerRanking, PlayerTier, RankingSystem};
pub use player::Player;
pub use player_list::{PlayerList, MAX_PLAYER_COUNT};
pub use player_template::{PlayerTemplate, PlayerTemplateStore};
pub use post_game_stats::{GameResult, PlayerPostGameStats, PlayerSide, PostGameStatistics};
pub use production_prerequisite::{PrereqUnitFlags, PrereqUnitRec, ProductionPrerequisite};
pub use resource_gathering_manager::{ResourceGatheringManager, ResourceWorld};
pub use science::{
    get_science_store, get_science_store_mut, init_science_store, ScienceAccess, ScienceInfo,
    ScienceStore, ScienceSubsystem, ScienceType, SCIENCE_INVALID,
};
pub use score_keeper::ScoreKeeper;
pub use special_power::{SpecialPowerStore, SpecialPowerTemplate, SpecialPowerType};
pub use team::{
    PlayerRef, Relationship, SidesListReader, Team, TeamFactory, TeamID, TeamInfoReader,
    TeamMember, TeamPrototype, TeamPrototypeFlags, TeamRelationMap, TEAM_ID_INVALID,
    TEAM_PROTOTYPE_ID_INVALID,
};
pub use tunnel_tracker::{TunnelDestroyResult, TunnelTracker, INVALID_ID};
