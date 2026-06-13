use game_engine::common::thing::{BuildCompletionType, ThingTemplate, ThingTrait};

#[derive(Debug, Clone)]
struct TestProductionThing {
    cost_percent: f32,
    time_percent: f32,
    kind_multiplier: f32,
    cost_handicap: f32,
    time_handicap: f32,
    energy_ratio: f32,
    instant: bool,
    facility_count: i32,
}

impl Default for TestProductionThing {
    fn default() -> Self {
        Self {
            cost_percent: 0.0,
            time_percent: 0.0,
            kind_multiplier: 1.0,
            cost_handicap: 1.0,
            time_handicap: 1.0,
            energy_ratio: 1.0,
            instant: false,
            facility_count: 0,
        }
    }
}

impl ThingTrait for TestProductionThing {
    fn get_production_cost_change_percent(&self, _template_name: &str) -> f32 {
        self.cost_percent
    }

    fn get_production_time_change_percent(&self, _template_name: &str) -> f32 {
        self.time_percent
    }

    fn get_production_cost_change_based_on_kind_of(&self, _kind_of: u64) -> f32 {
        self.kind_multiplier
    }

    fn get_build_cost_handicap(&self, _template: &ThingTemplate) -> f32 {
        self.cost_handicap
    }

    fn get_build_time_handicap(&self, _template: &ThingTemplate) -> f32 {
        self.time_handicap
    }

    fn get_energy_supply_ratio(&self) -> f32 {
        self.energy_ratio
    }

    fn builds_instantly_for_debug(&self) -> bool {
        self.instant
    }

    fn count_equivalent_build_facilities(&self, _template: &ThingTemplate) -> i32 {
        self.facility_count
    }
}

#[test]
fn calc_cost_to_build_applies_cpp_player_modifiers_without_min_clamp() {
    let mut template = ThingTemplate::new();
    template.set_template_name("TestTank".into());
    template.set_build_cost(1000);
    template.set_kindof_mask(0x10);

    let player = TestProductionThing {
        cost_percent: -0.25,
        kind_multiplier: 0.8,
        cost_handicap: 1.1,
        ..Default::default()
    };

    assert_eq!(template.calc_cost_to_build(Some(&player)), 660);
    assert_eq!(template.calc_cost_to_build(None), 0);

    let free_player = TestProductionThing {
        cost_percent: -0.99,
        kind_multiplier: 0.01,
        cost_handicap: 0.1,
        ..Default::default()
    };

    assert_eq!(template.calc_cost_to_build(Some(&free_player)), 0);
}

#[test]
fn calc_time_to_build_applies_cpp_player_modifiers_in_order() {
    let mut template = ThingTemplate::new();
    template.set_template_name("TestFactoryUnit".into());
    template.set_build_time(2.0);

    let player = TestProductionThing {
        time_percent: 0.25,
        time_handicap: 1.5,
        ..Default::default()
    };

    assert_eq!(template.calc_time_to_build(None), 60);
    assert_eq!(template.calc_time_to_build(Some(&player)), 112);
}

#[test]
fn calc_time_to_build_keeps_debug_instant_in_cpp_modifier_order() {
    let mut template = ThingTemplate::new();
    template.set_template_name("InstantUnit".into());
    template.set_build_time(10.0);
    template.set_build_completion(BuildCompletionType::AppearsAtRallyPoint);

    let player = TestProductionThing {
        instant: true,
        facility_count: 3,
        ..Default::default()
    };

    assert_eq!(template.calc_time_to_build(Some(&player)), 1);
}
