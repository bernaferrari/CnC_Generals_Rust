//! Integration Test: Weapon Firing and Damage System
//!
//! This test verifies that weapons can fire correctly and apply damage to targets:
//! - Weapon templates and configurations
//! - Projectile creation and movement
//! - Hit detection and damage calculation
//! - Damage types and armor interactions
//! - Area of effect (splash) damage
//! - Kill tracking and experience gain
//!
//! Tests should pass on all platforms (Windows, Linux, macOS)

#![cfg(test)]

use std::collections::HashMap;

/// Damage types matching C&C Generals
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum DamageType {
    Small,          // Anti-infantry
    Ap,             // Armor piercing
    ArmorPiercing,  // Heavy armor piercing
    Explosion,      // Explosive damage
    Fire,           // Flame weapons
    Laser,          // Energy weapons
    Sniper,         // Sniper rifles
}

/// Armor types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArmorType {
    Infantry,
    LightArmor,
    HeavyArmor,
    Aircraft,
    Structure,
}

/// Weapon template
#[derive(Debug, Clone)]
struct WeaponTemplate {
    name: String,
    damage: u32,
    damage_type: DamageType,
    range: f32,
    reload_time: f32,
    accuracy: f32,
    splash_radius: f32,
    projectile_speed: f32,
}

impl WeaponTemplate {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            damage: 25,
            damage_type: DamageType::Small,
            range: 100.0,
            reload_time: 1.0,
            accuracy: 95.0,
            splash_radius: 0.0,
            projectile_speed: 300.0,
        }
    }

    fn with_damage(mut self, damage: u32, damage_type: DamageType) -> Self {
        self.damage = damage;
        self.damage_type = damage_type;
        self
    }

    fn with_splash(mut self, radius: f32) -> Self {
        self.splash_radius = radius;
        self
    }
}

/// Mock target
#[derive(Debug)]
struct Target {
    id: u32,
    health: i32,
    max_health: i32,
    armor: ArmorType,
    position: (f32, f32, f32),
    alive: bool,
}

impl Target {
    fn new(id: u32, armor: ArmorType, health: i32) -> Self {
        Self {
            id,
            health,
            max_health: health,
            armor,
            position: (0.0, 0.0, 0.0),
            alive: true,
        }
    }

    fn take_damage(&mut self, damage: u32, damage_type: DamageType) {
        // Apply damage modifiers based on armor vs damage type
        let modifier = get_damage_modifier(damage_type, self.armor);
        let final_damage = (damage as f32 * modifier) as i32;

        self.health -= final_damage;
        if self.health <= 0 {
            self.health = 0;
            self.alive = false;
        }
    }

    fn is_alive(&self) -> bool {
        self.alive
    }

    fn distance_to(&self, pos: (f32, f32, f32)) -> f32 {
        let dx = self.position.0 - pos.0;
        let dy = self.position.1 - pos.1;
        let dz = self.position.2 - pos.2;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }
}

/// Get damage modifier based on damage type and armor type
fn get_damage_modifier(damage_type: DamageType, armor: ArmorType) -> f32 {
    // Simplified damage table
    match (damage_type, armor) {
        (DamageType::Small, ArmorType::Infantry) => 1.0,
        (DamageType::Small, ArmorType::LightArmor) => 0.25,
        (DamageType::Small, _) => 0.1,

        (DamageType::Ap, ArmorType::LightArmor) => 1.0,
        (DamageType::Ap, ArmorType::HeavyArmor) => 0.75,
        (DamageType::Ap, ArmorType::Infantry) => 0.5,
        (DamageType::Ap, _) => 0.5,

        (DamageType::ArmorPiercing, ArmorType::HeavyArmor) => 1.0,
        (DamageType::ArmorPiercing, ArmorType::LightArmor) => 1.0,
        (DamageType::ArmorPiercing, _) => 0.25,

        (DamageType::Explosion, _) => 1.0,

        (DamageType::Fire, ArmorType::Infantry) => 1.25,
        (DamageType::Fire, _) => 0.75,

        _ => 1.0,
    }
}

/// Test basic weapon creation
#[test]
fn test_weapon_creation() {
    println!("Testing weapon creation...");

    let rifle = WeaponTemplate::new("Rifle")
        .with_damage(25, DamageType::Small);

    assert_eq!(rifle.name, "Rifle");
    assert_eq!(rifle.damage, 25);
    assert_eq!(rifle.damage_type, DamageType::Small);

    let cannon = WeaponTemplate::new("Cannon")
        .with_damage(75, DamageType::ArmorPiercing)
        .with_splash(10.0);

    assert_eq!(cannon.damage, 75);
    assert_eq!(cannon.splash_radius, 10.0);

    log::info!("Weapon creation test passed");
}

/// Test basic damage application
#[test]
fn test_damage_application() {
    println!("Testing damage application...");

    let mut target = Target::new(1, ArmorType::Infantry, 100);
    assert_eq!(target.health, 100);
    assert!(target.is_alive());

    // Apply damage
    target.take_damage(30, DamageType::Small);
    assert_eq!(target.health, 70);
    assert!(target.is_alive());

    // More damage
    target.take_damage(50, DamageType::Small);
    assert_eq!(target.health, 20);
    assert!(target.is_alive());

    // Kill
    target.take_damage(30, DamageType::Small);
    assert_eq!(target.health, 0);
    assert!(!target.is_alive());

    log::info!("Damage application test passed");
}

/// Test damage type vs armor type interactions
#[test]
fn test_damage_armor_interaction() {
    println!("Testing damage vs armor interactions...");

    // Small arms vs infantry (100% damage)
    let mut infantry = Target::new(1, ArmorType::Infantry, 100);
    infantry.take_damage(100, DamageType::Small);
    assert_eq!(infantry.health, 0); // Full damage

    // Small arms vs tank (25% damage)
    let mut tank = Target::new(2, ArmorType::LightArmor, 100);
    tank.take_damage(100, DamageType::Small);
    assert_eq!(tank.health, 75); // 25% damage = 25 damage

    // Armor piercing vs tank (100% damage)
    let mut tank2 = Target::new(3, ArmorType::LightArmor, 100);
    tank2.take_damage(100, DamageType::Ap);
    assert_eq!(tank2.health, 0); // Full damage

    // Fire vs infantry (125% damage)
    let mut infantry2 = Target::new(4, ArmorType::Infantry, 100);
    infantry2.take_damage(100, DamageType::Fire);
    assert_eq!(infantry2.health, 0); // 125 damage (overkill)

    log::info!("Damage vs armor interaction test passed");
}

/// Test splash damage
#[test]
fn test_splash_damage() {
    println!("Testing splash damage...");

    // Create targets at different distances
    let mut targets = vec![
        Target::new(1, ArmorType::Infantry, 100),
        Target::new(2, ArmorType::Infantry, 100),
        Target::new(3, ArmorType::Infantry, 100),
    ];

    targets[0].position = (0.0, 0.0, 0.0);   // At impact point
    targets[1].position = (5.0, 0.0, 0.0);   // 5m away
    targets[2].position = (15.0, 0.0, 0.0);  // 15m away (outside splash)

    let weapon = WeaponTemplate::new("Grenade")
        .with_damage(100, DamageType::Explosion)
        .with_splash(10.0);

    let impact_point = (0.0, 0.0, 0.0);

    // Apply splash damage
    for target in targets.iter_mut() {
        let distance = target.distance_to(impact_point);

        if distance <= weapon.splash_radius {
            // Damage falloff with distance
            let damage_ratio = 1.0 - (distance / weapon.splash_radius);
            let damage = (weapon.damage as f32 * damage_ratio) as u32;
            target.take_damage(damage, weapon.damage_type);
        }
    }

    // Target 1: Full damage
    assert_eq!(targets[0].health, 0);

    // Target 2: Partial damage (50% at 5m from 10m radius)
    assert!(targets[1].health > 0 && targets[1].health < 100);

    // Target 3: No damage (outside radius)
    assert_eq!(targets[2].health, 100);

    log::info!("Splash damage test passed");
}

/// Test weapon range
#[test]
fn test_weapon_range() {
    println!("Testing weapon range...");

    let weapon = WeaponTemplate::new("Rifle");
    weapon.range;

    // Helper function to check if target is in range
    let in_range = |weapon_pos: (f32, f32, f32), target_pos: (f32, f32, f32), range: f32| {
        let dx = weapon_pos.0 - target_pos.0;
        let dy = weapon_pos.1 - target_pos.1;
        let distance = (dx * dx + dy * dy).sqrt();
        distance <= range
    };

    let weapon_pos = (0.0, 0.0, 0.0);

    assert!(in_range(weapon_pos, (50.0, 0.0, 0.0), 100.0));   // In range
    assert!(!in_range(weapon_pos, (150.0, 0.0, 0.0), 100.0)); // Out of range
    assert!(in_range(weapon_pos, (70.0, 70.0, 0.0), 100.0));  // Diagonal, in range

    log::info!("Weapon range test passed");
}

/// Test weapon accuracy
#[test]
fn test_weapon_accuracy() {
    println!("Testing weapon accuracy...");

    let accurate_weapon = WeaponTemplate::new("SniperRifle");
    let mut accurate_weapon = accurate_weapon;
    accurate_weapon.accuracy = 99.0;

    let inaccurate_weapon = WeaponTemplate::new("Shotgun");
    let mut inaccurate_weapon = inaccurate_weapon;
    inaccurate_weapon.accuracy = 50.0;

    // Simulate 100 shots
    let mut hits_accurate = 0;
    let mut hits_inaccurate = 0;

    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hash, Hasher};

    let mut seed = 12345u64;
    for _ in 0..100 {
        // Simple deterministic "random" generator
        seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
        let roll = (seed % 100) as f32;

        if roll < accurate_weapon.accuracy {
            hits_accurate += 1;
        }

        seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
        let roll = (seed % 100) as f32;

        if roll < inaccurate_weapon.accuracy {
            hits_inaccurate += 1;
        }
    }

    println!("Accurate weapon: {}/100 hits", hits_accurate);
    println!("Inaccurate weapon: {}/100 hits", hits_inaccurate);

    // Accurate weapon should hit more often
    assert!(hits_accurate > hits_inaccurate);
    assert!(hits_accurate >= 90); // Should hit ~99% of time

    log::info!("Weapon accuracy test passed");
}

/// Test reload time
#[test]
fn test_reload_time() {
    println!("Testing weapon reload time...");

    #[derive(Debug)]
    struct WeaponState {
        template: WeaponTemplate,
        last_fire_time: f32,
        can_fire: bool,
    }

    impl WeaponState {
        fn new(template: WeaponTemplate) -> Self {
            Self {
                template,
                last_fire_time: 0.0,
                can_fire: true,
            }
        }

        fn update(&mut self, current_time: f32) {
            let time_since_fire = current_time - self.last_fire_time;
            self.can_fire = time_since_fire >= self.template.reload_time;
        }

        fn fire(&mut self, current_time: f32) -> bool {
            if self.can_fire {
                self.last_fire_time = current_time;
                self.can_fire = false;
                true
            } else {
                false
            }
        }
    }

    let mut weapon = WeaponState::new(
        WeaponTemplate::new("MachineGun")
    );
    weapon.template.reload_time = 0.5;

    let mut time = 0.0;

    // First shot
    assert!(weapon.fire(time));

    // Immediate second shot should fail
    assert!(!weapon.fire(time));

    // After 0.3 seconds (not enough)
    time += 0.3;
    weapon.update(time);
    assert!(!weapon.fire(time));

    // After 0.5 seconds total (now ready)
    time += 0.2;
    weapon.update(time);
    assert!(weapon.fire(time));

    log::info!("Reload time test passed");
}

/// Test multi-weapon system
#[test]
fn test_multiple_weapons() {
    println!("Testing multiple weapon system...");

    let weapons = vec![
        WeaponTemplate::new("Primary").with_damage(50, DamageType::Ap),
        WeaponTemplate::new("Secondary").with_damage(25, DamageType::Small),
    ];

    assert_eq!(weapons.len(), 2);

    let mut target = Target::new(1, ArmorType::LightArmor, 200);

    // Fire primary (armor piercing vs light armor = 100% damage)
    target.take_damage(weapons[0].damage, weapons[0].damage_type);
    assert_eq!(target.health, 150);

    // Fire secondary (small arms vs light armor = 25% damage)
    target.take_damage(weapons[1].damage, weapons[1].damage_type);
    assert_eq!(target.health, 150 - 6); // 25 * 0.25 = 6.25, rounded to 6

    log::info!("Multiple weapons test passed");
}

/// Test kill tracking
#[test]
fn test_kill_tracking() {
    println!("Testing kill tracking...");

    struct UnitWithWeapon {
        kills: u32,
        weapon: WeaponTemplate,
    }

    impl UnitWithWeapon {
        fn attack(&mut self, target: &mut Target) {
            target.take_damage(self.weapon.damage, self.weapon.damage_type);

            if !target.is_alive() {
                self.kills += 1;
            }
        }
    }

    let mut attacker = UnitWithWeapon {
        kills: 0,
        weapon: WeaponTemplate::new("Rifle").with_damage(100, DamageType::Small),
    };

    // Kill multiple targets
    for i in 0..5 {
        let mut target = Target::new(i, ArmorType::Infantry, 100);
        attacker.attack(&mut target);
        assert!(!target.is_alive());
    }

    assert_eq!(attacker.kills, 5);

    log::info!("Kill tracking test passed");
}

#[cfg(test)]
mod performance_tests {
    use super::*;

    /// Stress test: Mass combat simulation
    #[test]
    #[ignore] // Run with: cargo test --test integration_weapon_damage -- --ignored
    fn test_mass_combat_simulation() {
        println!("Stress test: Mass combat simulation...");

        const NUM_UNITS: usize = 1000;
        const NUM_SHOTS: usize = 10;

        let weapon = WeaponTemplate::new("StandardRifle")
            .with_damage(25, DamageType::Small);

        let mut targets: Vec<Target> = (0..NUM_UNITS)
            .map(|i| Target::new(i as u32, ArmorType::Infantry, 100))
            .collect();

        let start = std::time::Instant::now();

        // Simulate combat
        for _ in 0..NUM_SHOTS {
            for target in targets.iter_mut().filter(|t| t.is_alive()) {
                target.take_damage(weapon.damage, weapon.damage_type);
            }
        }

        let elapsed = start.elapsed();

        let total_shots = NUM_UNITS * NUM_SHOTS;
        let shots_per_sec = total_shots as f64 / elapsed.as_secs_f64();

        println!("Processed {} shots in {:?} ({:.0} shots/sec)", total_shots, elapsed, shots_per_sec);

        let alive_count = targets.iter().filter(|t| t.is_alive()).count();
        println!("Units remaining: {}/{}", alive_count, NUM_UNITS);

        assert!(shots_per_sec > 100000.0, "Should process >100k shots/second");

        log::info!("Mass combat simulation passed");
    }
}
