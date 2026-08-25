use super::super::super::*;

impl GameLogic {
    // -----------------------------------------------------------------------
    // Mine / demo-trap / timed demo-charge residual
    // Fail-closed: not full MinefieldBehavior / DemoTrapUpdate / StickyBombUpdate.
    // -----------------------------------------------------------------------

    /// Residual honesty: at least one mine/trap/charge was placed.
    pub fn mine_residual_places(&self) -> u32 {
        self.mine_residual_places
    }

    /// Residual honesty: proximity-triggered detonations.
    pub fn mine_residual_proximity_detonations(&self) -> u32 {
        self.mine_residual_proximity_detonations
    }

    /// Residual honesty: timed-charge detonations.
    pub fn mine_residual_timed_detonations(&self) -> u32 {
        self.mine_residual_timed_detonations
    }

    /// Residual honesty: manual detonations (demo trap command residual).
    pub fn mine_residual_manual_detonations(&self) -> u32 {
        self.mine_residual_manual_detonations
    }

    /// Residual honesty: dozer/worker safe mine clears (disarm without detonation).
    pub fn mine_residual_clears(&self) -> u32 {
        self.mine_residual_clears
    }

    /// Residual honesty: place → enemy trigger → damage path exercised.
    pub fn honesty_mine_place_trigger_ok(&self) -> bool {
        self.mine_residual_places > 0 && self.mine_residual_proximity_detonations > 0
    }

    /// Residual honesty: place timed charge → detonation path exercised.
    pub fn honesty_timed_demo_charge_ok(&self) -> bool {
        self.mine_residual_places > 0 && self.mine_residual_timed_detonations > 0
    }

    /// Residual honesty: place enemy mine → dozer clear → mine gone, dozer lives.
    pub fn honesty_mine_clear_ok(&self) -> bool {
        self.mine_residual_places > 0 && self.mine_residual_clears > 0
    }

    /// Residual dozer structure-repair command accepts.
    pub fn repair_residual_structure_commands(&self) -> u32 {
        self.repair_residual_structure_commands
    }

    /// Residual structure HP heal ticks applied by dozer Repairing state.
    pub fn repair_residual_structure_heals(&self) -> u32 {
        self.repair_residual_structure_heals
    }

    /// Residual vehicle/aircraft SeekingRepair heal ticks at pad/war-factory/airfield.
    pub fn repair_residual_vehicle_heals(&self) -> u32 {
        self.repair_residual_vehicle_heals
    }

    /// Record a successful dozer structure Repair command acceptance.
    pub fn record_structure_repair_residual_command(&mut self) {
        self.repair_residual_structure_commands =
            self.repair_residual_structure_commands.saturating_add(1);
    }

    /// Record a structure HP heal tick from dozer Repairing residual.
    pub fn record_structure_repair_residual_heal(&mut self) {
        self.repair_residual_structure_heals =
            self.repair_residual_structure_heals.saturating_add(1);
    }

    /// Record a vehicle/aircraft pad heal tick from SeekingRepair residual.
    pub fn record_vehicle_repair_residual_heal(&mut self) {
        self.repair_residual_vehicle_heals = self.repair_residual_vehicle_heals.saturating_add(1);
    }

    /// Residual structure repair honesty: command issued and at least one HP heal tick.
    /// Fail-closed: not full C++ percent-heal / sole-benefactor / scaffolding parity.
    pub fn honesty_structure_repair_ok(&self) -> bool {
        self.repair_residual_structure_commands > 0 && self.repair_residual_structure_heals > 0
    }

    /// Residual vehicle pad repair honesty: at least one SeekingRepair heal tick.
    /// Fail-closed: not full RepairDockUpdate TimeForFullHeal / dock bones parity.
    pub fn honesty_vehicle_repair_ok(&self) -> bool {
        self.repair_residual_vehicle_heals > 0
    }

    /// Combined host repair residual path honesty (structure or vehicle pad).
    pub fn honesty_repair_ok(&self) -> bool {
        self.honesty_structure_repair_ok() || self.honesty_vehicle_repair_ok()
    }

    /// Residual ambulance AutoHeal infantry HP ticks applied.
    pub fn heal_residual_ambulance_heals(&self) -> u32 {
        self.heal_residual_ambulance_heals
    }

    /// Residual HealPad SeekingHealing HP ticks applied.
    pub fn heal_residual_heal_pad_heals(&self) -> u32 {
        self.heal_residual_heal_pad_heals
    }

    /// Record an ambulance radius AutoHeal infantry HP tick.
    pub fn record_ambulance_residual_heal(&mut self) {
        self.heal_residual_ambulance_heals = self.heal_residual_ambulance_heals.saturating_add(1);
    }

    /// Record a HealPad SeekingHealing HP tick.
    pub fn record_heal_pad_residual_heal(&mut self) {
        self.heal_residual_heal_pad_heals = self.heal_residual_heal_pad_heals.saturating_add(1);
    }

    /// Residual ambulance infantry heal honesty: at least one radius AutoHeal tick.
    /// Fail-closed: not full sole-benefactor / vehicle AutoHeal ModuleTag_23 parity.
    pub fn honesty_ambulance_heal_ok(&self) -> bool {
        self.heal_residual_ambulance_heals > 0
    }

    /// Residual HealPad infantry heal honesty: at least one SeekingHealing tick.
    pub fn honesty_heal_pad_ok(&self) -> bool {
        self.heal_residual_heal_pad_heals > 0
    }

    /// Combined host infantry heal residual honesty (ambulance radius or HealPad).
    pub fn honesty_heal_ok(&self) -> bool {
        self.honesty_ambulance_heal_ok() || self.honesty_heal_pad_ok()
    }

    /// Host propaganda tower residual heal honesty ticks.
    pub fn propaganda_residual_heals(&self) -> u32 {
        self.propaganda_residual_heals
    }

    /// Host propaganda tower residual buff honesty ticks.
    pub fn propaganda_residual_buffs(&self) -> u32 {
        self.propaganda_residual_buffs
    }

    pub(in crate::game_logic) fn record_propaganda_residual_heal(&mut self) {
        self.propaganda_residual_heals = self.propaganda_residual_heals.saturating_add(1);
    }

    pub(in crate::game_logic) fn record_propaganda_residual_buff(&mut self) {
        self.propaganda_residual_buffs = self.propaganda_residual_buffs.saturating_add(1);
    }

    /// Residual honesty: speaker/propaganda tower healed at least one unit.
    pub fn honesty_propaganda_heal_ok(&self) -> bool {
        self.propaganda_residual_heals > 0
    }

    /// Residual honesty: speaker/propaganda tower granted ENTHUSIASTIC/SUBLIMINAL buff.
    pub fn honesty_propaganda_buff_ok(&self) -> bool {
        self.propaganda_residual_buffs > 0
    }

    /// Combined host propaganda tower residual honesty (heal or buff).
    pub fn honesty_propaganda_ok(&self) -> bool {
        self.honesty_propaganda_heal_ok() || self.honesty_propaganda_buff_ok()
    }

    /// Host ECM tank residual jam honesty ticks (DISABLED_SUBDUED grants).
    pub fn ecm_residual_jams(&self) -> u32 {
        self.ecm_residual_jams
    }

    pub(in crate::game_logic) fn record_ecm_residual_jam(&mut self) {
        self.ecm_residual_jams = self.ecm_residual_jams.saturating_add(1);
    }

    /// Residual honesty: ECM tank / jammer jammed enemy weapons at least once.
    pub fn honesty_ecm_jam_ok(&self) -> bool {
        self.ecm_residual_jams > 0
            || self.ecm_missiles_jammed > 0
            || self.ecm_laser_beams_spawned > 0
    }

    /// Residual honesty: ECMDisableStream laser spawned at least once.
    pub fn honesty_ecm_laser_ok(&self) -> bool {
        self.ecm_laser_beams_spawned > 0
    }

    /// Host Microwave Tank residual registry (disable structure honesty).
    pub fn microwave_residual(&self) -> &crate::game_logic::host_microwave::HostMicrowaveRegistry {
        &self.microwaves
    }

    /// Residual honesty: Microwave tank disabled an enemy structure at least once.
    pub fn honesty_microwave_disable_ok(&self) -> bool {
        self.microwaves.honesty_disable_ok()
    }

    /// Residual honesty: MicrowaveDisableStream laser spawned at least once.
    pub fn honesty_microwave_laser_ok(&self) -> bool {
        self.microwaves.honesty_laser_ok()
    }

    /// Residual honesty: emitter MICROWAVE field damaged at least once.
    pub fn honesty_microwave_emitter_ok(&self) -> bool {
        self.microwaves.honesty_emitter_ok()
    }

    /// Combined host path honesty for Microwave residual (disable).
    /// Garrison clear honesty is tracked separately via `honesty_kill_garrisoned_ok`.
    pub fn honesty_microwave_ok(&self) -> bool {
        self.microwaves.honesty_disable_ok()
            || self.microwaves.honesty_laser_ok()
            || self.microwaves.honesty_emitter_ok()
    }

    /// Host EMP Pulse residual registry (activate + honesty).
    pub fn emp_pulses(&self) -> &crate::game_logic::host_emp_pulse::HostEmpPulseRegistry {
        &self.emp_pulses
    }

    /// Residual honesty: EmpPulse activated at least once.
    pub fn honesty_emp_pulse_activate_ok(&self) -> bool {
        self.emp_pulses.honesty_activate_ok()
    }

    /// Residual honesty: EmpPulse applied DISABLED_EMP at least once.
    pub fn honesty_emp_pulse_disable_ok(&self) -> bool {
        self.emp_pulses.honesty_disable_ok()
    }

    /// Combined host path honesty for EmpPulse residual.
    pub fn honesty_emp_pulse_ok(&self) -> bool {
        self.emp_pulses.honesty_host_path_ok()
    }
}
