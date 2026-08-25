use super::*;

impl Player {
    // =========================================================
    // AI Build Commands (C++ Player.cpp lines 1858-1960)
    /// Build specific team (AI command)
    /// C++ Reference: Player::buildSpecificTeam() (Player.cpp lines 2309-2316)
    pub fn build_specific_team(&mut self, team_name: &str) {
        if let Some(ai) = self.get_ai() {
            ai.build_specific_ai_team(team_name, true);
        }
    }

    /// Build base defense (AI command)
    /// C++ Reference: Player::buildBaseDefense() (Player.cpp lines 2319-2326)
    pub fn build_base_defense(&mut self, flank: bool) {
        if let Some(ai) = self.get_ai() {
            ai.build_ai_base_defense(flank);
        }
    }

    /// Build base defense structure (AI command)
    /// C++ Reference: Player::buildBaseDefenseStructure() (Player.cpp lines 2329-2336)
    pub fn build_base_defense_structure(&mut self, thing_name: &str, flank: bool) {
        if let Some(ai) = self.get_ai() {
            ai.build_ai_base_defense_structure(thing_name, flank);
        }
    }

    /// Build specific building (AI command)
    /// C++ Reference: Player::buildSpecificBuilding() (Player.cpp lines 2339-2346)
    pub fn build_specific_building(&mut self, thing_name: &str) {
        if let Some(ai) = self.get_ai() {
            ai.build_specific_ai_building(thing_name);
        }
    }

    /// Build by supplies (AI command)
    /// C++ Reference: Player::buildBySupplies() (Player.cpp lines 2349-2355)
    pub fn build_by_supplies(&mut self, minimum_cash: i32, thing_name: &str) {
        if let Some(ai) = self.get_ai() {
            ai.build_by_supplies(minimum_cash, thing_name);
        }
    }

    /// Build specific building nearest team (AI command)
    /// C++ Reference: Player::buildSpecificBuildingNearestTeam() (Player.cpp lines 2358-2364)
    pub fn build_specific_building_nearest_team(&mut self, thing_name: &str, team_id: i32) {
        if let Some(ai) = self.get_ai() {
            ai.build_specific_building_nearest_team(thing_name, team_id);
        }
    }

    /// Build upgrade (AI command)
    /// C++ Reference: Player::buildUpgrade() (Player.cpp lines 2367-2373)
    pub fn build_upgrade(&mut self, upgrade_name: &str) {
        if let Some(ai) = self.get_ai() {
            ai.build_upgrade(upgrade_name);
        }
    }

    /// Recruit specific team (AI command)
    /// C++ Reference: Player::recruitSpecificTeam() (Player.cpp lines 2376-2383)
    pub fn recruit_specific_team(&mut self, team_name: &str, recruit_radius: f32) {
        if let Some(ai) = self.get_ai() {
            ai.recruit_specific_ai_team(team_name, recruit_radius);
        }
    }

    /// Calculate closest construction zone location
    /// C++ Reference: Player::calcClosestConstructionZoneLocation() (Player.cpp lines 2389-2397)
    pub fn calc_closest_construction_zone(&self, template_name: &str) -> Option<Coord3D> {
        self.get_ai()
            .and_then(|ai| ai.calc_closest_construction_zone(template_name))
    }

    // =========================================================
}
