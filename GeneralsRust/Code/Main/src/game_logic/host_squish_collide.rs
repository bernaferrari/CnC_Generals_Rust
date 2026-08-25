//! Host SquishCollide residual (infantry crushed under vehicle wheels).
//!
//! C++: `SquishCollide::onCollide` — when a crusher (other) overlaps this object
//! with crusherLevel > 0, non-ally, geom hit at 1.0 crush radius, and velocity
//! points toward victim → DAMAGE_CRUSH / DEATH_CRUSHED with HUGE_DAMAGE_AMOUNT.
//!
//! - Velocity-toward-victim gate (dot product residual)
//! - Ally skip residual
//! - Goal-object skip only when AI `getGoalObject() == crusher` AND
//!   (`HijackerUpdate` exists OR `SPECIAL_TANKHUNTER_TNT_ATTACK` is active)
//! - Victim geom forced to 1.0 major/minor then `geomCollidesWithGeom`
//!
//! Fail-closed: HijackerUpdate presence is template-module peel (C++
//! `findUpdateModule("HijackerUpdate")`); TNT-active is host SpecialAbility
//! state on the victim.

use serde::{Deserialize, Serialize};

/// C++ HUGE_DAMAGE_AMOUNT residual for squish kill.
pub const SQUISH_HUGE_DAMAGE: f32 = 999_999.0;
/// C++ crush geometry radius residual (major/minor forced to 1.0).
pub const SQUISH_CRUSH_RADIUS: f32 = 1.0;

/// True if crusher velocity points toward victim (C++ to·vel > 0).
///
/// `crusher_pos` / `victim_pos` are XZ world; `vel` is crusher velocity XZ.
pub fn velocity_toward_victim(
    crusher_pos: (f32, f32),
    victim_pos: (f32, f32),
    vel: (f32, f32),
) -> bool {
    let to_x = victim_pos.0 - crusher_pos.0;
    let to_z = victim_pos.1 - crusher_pos.1;
    to_x * vel.0 + to_z * vel.1 > 0.0
}

/// Tight crush radius residual: victim selection radius clamped for hit test.
pub fn squish_hit_radius(selection_radius: f32) -> f32 {
    // C++ forces major/minor to 1.0 for the victim geom during collide test.
    let _ = selection_radius;
    SQUISH_CRUSH_RADIUS
}

/// C++ `SquishCollide.cpp:79-86` — copy victim GeometryInfo, force
/// `setMajorRadius(1.0f)` / `setMinorRadius(1.0f)`, then
/// `ThePartitionManager->geomCollidesWithGeom` against the crusher's real
/// geometry and orientation. A 1-world-unit victim disk must actually overlap.
///
/// Positions are host Y-up `(x, y, z)`; collide space is C++ Z-up.
pub fn squish_geom_collides(
    crusher_pos: (f32, f32, f32),
    crusher_angle: f32,
    crusher_major: f32,
    crusher_height: f32,
    victim_pos: (f32, f32, f32),
    victim_angle: f32,
    victim_height: f32,
) -> bool {
    use gamelogic::object::collide::GeometryInfo;
    squish_geom_collides_with(
        crusher_pos,
        crusher_angle,
        GeometryInfo::new_cylinder(
            crusher_major.max(1.0),
            crusher_height.max(1.0),
            crusher_major <= 20.0,
        ),
        victim_pos,
        victim_angle,
        victim_height,
    )
}

/// C++ `SquishCollide.cpp:83` — crusher uses **real** `GeometryInfo` (BOX for
/// tanks), not a cylinder of the bounding/selection radius.
pub fn authored_crusher_geometry(
    info: &crate::game_logic::HostGeometryInfo,
    fallback_major: f32,
    fallback_height: f32,
) -> gamelogic::object::collide::GeometryInfo {
    use crate::game_logic::HostGeometryType;
    use gamelogic::object::collide::GeometryInfo;
    if info.authored {
        match info.geom_type {
            HostGeometryType::Sphere => GeometryInfo::new_sphere(info.major_radius, info.is_small),
            HostGeometryType::Cylinder => {
                GeometryInfo::new_cylinder(info.major_radius, info.height, info.is_small)
            }
            HostGeometryType::Box => {
                let mut geom = GeometryInfo::new_box(
                    info.major_radius * 2.0,
                    info.minor_radius * 2.0,
                    info.is_small,
                );
                geom.set_height(info.height);
                geom
            }
        }
    } else {
        GeometryInfo::new_cylinder(
            fallback_major.max(1.0),
            fallback_height.max(1.0),
            fallback_major <= 20.0,
        )
    }
}

/// C++ `geomCollidesWithGeom(other->getGeometryInfo(), victim 1.0 major/minor)`.
pub fn squish_geom_collides_with(
    crusher_pos: (f32, f32, f32),
    crusher_angle: f32,
    crusher_geom: gamelogic::object::collide::GeometryInfo,
    victim_pos: (f32, f32, f32),
    victim_angle: f32,
    victim_height: f32,
) -> bool {
    use gamelogic::object::collide::{
        CollideInfo, CollideLocAndNormal, Coord3D, GeometryInfo, collide_test_dispatch,
    };
    let geom_victim =
        GeometryInfo::new_cylinder(SQUISH_CRUSH_RADIUS, victim_height.max(0.01), true);
    let info_a = CollideInfo::new(
        Coord3D::new(crusher_pos.0, crusher_pos.2, crusher_pos.1),
        crusher_geom,
        crusher_angle,
    );
    let info_b = CollideInfo::new(
        Coord3D::new(victim_pos.0, victim_pos.2, victim_pos.1),
        geom_victim,
        victim_angle,
    );
    let this_top = info_a.position.z + info_a.geom.get_max_height_above_position();
    let this_bot = info_a.position.z - info_a.geom.get_max_height_below_position();
    let that_top = info_b.position.z + info_b.geom.get_max_height_above_position();
    let that_bot = info_b.position.z - info_b.geom.get_max_height_below_position();
    if this_top < that_bot || this_bot > that_top {
        return false;
    }
    let mut cinfo =
        CollideLocAndNormal::new(Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(0.0, 0.0, 0.0));
    collide_test_dispatch(
        crusher_geom.get_geom_type(),
        geom_victim.get_geom_type(),
        &info_a,
        &info_b,
        Some(&mut cinfo),
    )
}

/// Legacy 2D circle helper (tests / residuals). Live crush uses
/// [`squish_geom_collides`] (C++ 1.0 victim geom).
pub fn within_squish_radius(
    crusher_pos: (f32, f32),
    victim_pos: (f32, f32),
    crusher_radius: f32,
) -> bool {
    let _ = crusher_radius;
    let dx = crusher_pos.0 - victim_pos.0;
    let dz = crusher_pos.1 - victim_pos.1;
    let dist = (dx * dx + dz * dz).sqrt();
    // C++ victim disk is 1.0; crusher still uses its real geom. This helper
    // is not the live path — keep a 1.0 victim-only radius for residuals.
    dist <= SQUISH_CRUSH_RADIUS
}

/// C++ `findUpdateModule("HijackerUpdate")` residual: module is authored on
/// hijacker infantry templates.
pub fn template_has_hijacker_update(victim_template: &str) -> bool {
    victim_template.to_ascii_lowercase().contains("hijacker")
}

/// C++ `findModule("SquishCollide")` (`Object.cpp:1133`). Authored on infantry
/// Object INI; never inferred from KindOf.
pub fn template_has_squish_collide(template_name: &str) -> bool {
    let Some(manager) = crate::assets::get_asset_manager() else {
        return false;
    };
    let Ok(guard) = manager.lock() else {
        return false;
    };
    let Some(definition) = guard.get_object_definition(template_name) else {
        return false;
    };
    definition
        .behavior_modules
        .iter()
        .any(|module| module.class_name.eq_ignore_ascii_case("SquishCollide"))
}

/// C++ `SquishCollide::onCollide` (`SquishCollide.cpp:51-70`): skip crush only
/// when the victim's AI `getGoalObject() == other` AND (`HijackerUpdate` exists
/// OR `findSpecialAbilityUpdate(SPECIAL_TANKHUNTER_TNT_ATTACK)` is active).
/// A hijacker / tank hunter not targeting the crusher is still squished.
pub fn should_skip_squish_for_goal_ability(
    victim_goal: Option<super::ObjectId>,
    crusher_id: super::ObjectId,
    has_hijacker_update: bool,
    tnt_ability_active: bool,
) -> bool {
    victim_goal == Some(crusher_id) && (has_hijacker_update || tnt_ability_active)
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostSquishCollideLog {
    pub squish_kills: u32,
    pub velocity_rejects: u32,
    pub ally_rejects: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::ObjectId;

    #[test]
    fn toward_and_away() {
        // Crusher at 0 moving +X, victim at +5 → toward.
        assert!(velocity_toward_victim((0.0, 0.0), (5.0, 0.0), (2.0, 0.0)));
        // Moving -X away from victim at +5.
        assert!(!velocity_toward_victim((0.0, 0.0), (5.0, 0.0), (-2.0, 0.0)));
    }

    #[test]
    fn skip_only_when_goal_matches_and_ability() {
        // C++ SquishCollide.cpp:51-70 — template peel alone is not a skip.
        let crusher = ObjectId(7);
        assert!(!should_skip_squish_for_goal_ability(
            None, crusher, true, false
        ));
        assert!(!should_skip_squish_for_goal_ability(
            Some(ObjectId(99)),
            crusher,
            true,
            true
        ));
        assert!(should_skip_squish_for_goal_ability(
            Some(crusher),
            crusher,
            true,
            false
        ));
        assert!(should_skip_squish_for_goal_ability(
            Some(crusher),
            crusher,
            false,
            true
        ));
        assert!(!should_skip_squish_for_goal_ability(
            Some(crusher),
            crusher,
            false,
            false
        ));
        assert!(template_has_hijacker_update("GLAInfantryHijacker"));
        assert!(!template_has_hijacker_update("GLAInfantryRebel"));
        assert!(!template_has_hijacker_update("ChinaInfantryTankHunter"));
    }

    #[test]
    fn victim_geom_is_one_wu_not_selection_radius() {
        // C++ SquishCollide.cpp:79-86 — 1.0 victim disk, not selection+1.
        assert!((squish_hit_radius(16.0) - 1.0).abs() < f32::EPSILON);
        // Circle helper no longer inflates by crusher selection radius.
        assert!(within_squish_radius((0.0, 0.0), (0.5, 0.0), 16.0));
        assert!(!within_squish_radius((0.0, 0.0), (3.0, 0.0), 16.0));
        // Tank at origin with 8wu major; infantry far away is outside 1.0 victim.
        assert!(!squish_geom_collides(
            (0.0, 0.0, 0.0),
            0.0,
            8.0,
            5.0,
            (10.0, 0.0, 0.0),
            0.0,
            2.0,
        ));
        // Overlap: victim inside the 1.0 disk plus tank body.
        assert!(squish_geom_collides(
            (0.0, 0.0, 0.0),
            0.0,
            8.0,
            5.0,
            (8.5, 0.0, 0.0),
            0.0,
            2.0,
        ));
    }

    #[test]
    fn crusher_uses_authored_box_not_cylinder() {
        use gamelogic::object::collide::GeometryInfo;
        // Long thin BOX: major 8 (X), minor 2 (Z). Side point is outside BOX
        // but inside a cylinder of the major/selection radius.
        let mut box_geom = GeometryInfo::new_box(16.0, 4.0, true);
        box_geom.set_height(5.0);
        assert!(!squish_geom_collides_with(
            (0.0, 0.0, 0.0),
            0.0,
            box_geom,
            (0.0, 0.0, 6.0),
            0.0,
            2.0,
        ));
        assert!(squish_geom_collides(
            (0.0, 0.0, 0.0),
            0.0,
            8.0,
            5.0,
            (0.0, 0.0, 6.0),
            0.0,
            2.0,
        ));
        let mut along = GeometryInfo::new_box(16.0, 4.0, true);
        along.set_height(5.0);
        assert!(squish_geom_collides_with(
            (0.0, 0.0, 0.0),
            0.0,
            along,
            (8.5, 0.0, 0.0),
            0.0,
            2.0,
        ));
    }
}
