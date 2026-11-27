//! Rank progression rules shared by script and gameplay skill awards.

/// C++ GameLogic::reset default for the match's rank cap.
pub const DEFAULT_RANK_LEVEL_LIMIT: i32 = 1000;

/// C++ Player::addSkillPoints applies the player's modifier before capping the award.
/// A zero adjusted award leaves the current points intact, even after the cap changes.
pub fn skill_points_after_award(
    current: i32,
    delta: i32,
    modifier: f32,
    point_cap: i32,
) -> Option<i32> {
    let delta = game_engine::base_type::real_to_int_ceil(modifier * delta as f32);
    (delta != 0).then(|| current.saturating_add(delta).min(point_cap))
}
