//! Targets and roster rules for C++ ScriptActions.cpp:2842–2887.

use glam::Vec3;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RadarEventTarget<'a> {
    /// Authored X/Y map coordinates with Z as elevation.
    Position(Vec3),
    Object(&'a str),
    Team(&'a str),
}

/// HasAnyUnits is an eligibility check. GetEstimateTeamPosition still uses
/// the first roster member, including when that member is a structure.
pub fn team_event_position(
    members: impl IntoIterator<Item = Option<(Vec3, bool)>>,
) -> Option<Vec3> {
    let mut members = members.into_iter();
    let (first_position, first_is_unit) = members.next()??;
    (first_is_unit || members.flatten().any(|(_, is_unit)| is_unit)).then_some(first_position)
}
