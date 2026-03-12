//! AI benchmarking module
//!
//! Comprehensive AI performance benchmarks for game AI systems.
//! Tests pathfinding, decision trees, behavior trees, and strategic AI.
//!
//! # Benchmarks Included
//!
//! - **A* Pathfinding** - Grid-based pathfinding performance
//! - **Decision Trees** - Unit behavior decision making
//! - **State Machines** - FSM update performance
//! - **Behavior Trees** - Complex AI behavior execution
//! - **Formation Logic** - Unit group coordination
//! - **Target Selection** - Enemy targeting algorithms
//! - **Tactical Analysis** - Map analysis and strategy

use crate::{BenchmarkConfig, BenchmarkResult, BenchmarkCategory, Measurement, MeasurementUnit, Result};
use std::time::Instant;
use std::collections::{HashMap, BinaryHeap, VecDeque};
use std::cmp::Ordering;

/// AI benchmarks for game intelligence systems
pub struct AiBenchmarks {
    config: BenchmarkConfig,
}

impl AiBenchmarks {
    pub fn new(config: &BenchmarkConfig) -> Self {
        Self {
            config: config.clone(),
        }
    }

    /// Run all AI benchmarks
    pub async fn run_all(&mut self) -> Result<Vec<BenchmarkResult>> {
        let mut results = Vec::new();

        log::info!("Running AI benchmarks...");

        // Core AI algorithms
        results.push(self.benchmark_pathfinding_small_map().await?);
        results.push(self.benchmark_pathfinding_large_map().await?);
        results.push(self.benchmark_decision_trees().await?);
        results.push(self.benchmark_state_machines().await?);
        results.push(self.benchmark_behavior_trees().await?);
        results.push(self.benchmark_target_selection().await?);
        results.push(self.benchmark_formation_logic().await?);
        results.push(self.benchmark_tactical_analysis().await?);

        log::info!("AI benchmarks completed: {} tests", results.len());

        Ok(results)
    }

    /// Benchmark A* pathfinding on small map
    async fn benchmark_pathfinding_small_map(&self) -> Result<BenchmarkResult> {
        let mut result = BenchmarkResult::new(
            "AI Pathfinding (Small 50x50 Map)".to_string(),
            BenchmarkCategory::Ai,
        );

        result.add_metadata("description".to_string(),
            "Tests A* pathfinding on small tactical map".to_string());

        const SIZE: usize = 50;
        let grid = Self::create_test_grid(SIZE, 0.15); // 15% obstacles

        // Warmup
        for _ in 0..5 {
            let _ = Self::astar_pathfind(&grid, (0, 0), (SIZE - 1, SIZE - 1));
        }

        // Measure
        for _ in 0..self.config.measurement_iterations.min(50) {
            let start = Instant::now();
            let path = Self::astar_pathfind(&grid, (0, 0), (SIZE - 1, SIZE - 1));
            let duration = start.elapsed();

            result.add_measurement(Measurement::new(
                duration.as_micros() as f64,
                MeasurementUnit::Microseconds,
            ));

            if let Some(p) = path {
                result.add_metadata("path_length".to_string(), p.len().to_string());
            }
        }

        Ok(result)
    }

    /// Benchmark A* pathfinding on large map
    async fn benchmark_pathfinding_large_map(&self) -> Result<BenchmarkResult> {
        let mut result = BenchmarkResult::new(
            "AI Pathfinding (Large 200x200 Map)".to_string(),
            BenchmarkCategory::Ai,
        );

        result.add_metadata("description".to_string(),
            "Tests A* pathfinding on large strategic map".to_string());

        const SIZE: usize = 200;
        let grid = Self::create_test_grid(SIZE, 0.20); // 20% obstacles

        // Warmup
        for _ in 0..3 {
            let _ = Self::astar_pathfind(&grid, (0, 0), (SIZE - 1, SIZE - 1));
        }

        // Measure
        for _ in 0..self.config.measurement_iterations.min(20) {
            let start = Instant::now();
            let path = Self::astar_pathfind(&grid, (0, 0), (SIZE - 1, SIZE - 1));
            let duration = start.elapsed();

            result.add_measurement(Measurement::new(
                duration.as_millis() as f64,
                MeasurementUnit::Milliseconds,
            ));

            if let Some(p) = path {
                result.add_metadata("path_length".to_string(), p.len().to_string());
            }
        }

        Ok(result)
    }

    /// Create a test grid with obstacles
    fn create_test_grid(size: usize, obstacle_ratio: f64) -> Vec<Vec<bool>> {
        let mut grid = vec![vec![true; size]; size];

        // Add obstacles in a pattern
        for i in 0..size {
            for j in 0..size {
                let hash = ((i * 31 + j * 17) % 100) as f64 / 100.0;
                if hash < obstacle_ratio {
                    grid[i][j] = false; // obstacle
                }
            }
        }

        // Ensure start and end are walkable
        grid[0][0] = true;
        grid[size - 1][size - 1] = true;

        grid
    }

    /// A* pathfinding algorithm
    fn astar_pathfind(grid: &[Vec<bool>], start: (usize, usize), goal: (usize, usize)) -> Option<Vec<(usize, usize)>> {
        #[derive(Eq, PartialEq)]
        struct Node {
            pos: (usize, usize),
            f_score: i32,
        }

        impl Ord for Node {
            fn cmp(&self, other: &Self) -> Ordering {
                other.f_score.cmp(&self.f_score)
            }
        }

        impl PartialOrd for Node {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                Some(self.cmp(other))
            }
        }

        let heuristic = |a: (usize, usize), b: (usize, usize)| -> i32 {
            ((a.0 as i32 - b.0 as i32).abs() + (a.1 as i32 - b.1 as i32).abs()) * 10
        };

        let mut open_set = BinaryHeap::new();
        let mut came_from: HashMap<(usize, usize), (usize, usize)> = HashMap::new();
        let mut g_score: HashMap<(usize, usize), i32> = HashMap::new();

        g_score.insert(start, 0);
        open_set.push(Node {
            pos: start,
            f_score: heuristic(start, goal),
        });

        while let Some(Node { pos: current, .. }) = open_set.pop() {
            if current == goal {
                let mut path = vec![current];
                let mut current = current;
                while let Some(&prev) = came_from.get(&current) {
                    path.push(prev);
                    current = prev;
                }
                path.reverse();
                return Some(path);
            }

            let neighbors = [
                (current.0.wrapping_sub(1), current.1),
                (current.0 + 1, current.1),
                (current.0, current.1.wrapping_sub(1)),
                (current.0, current.1 + 1),
            ];

            for neighbor in neighbors {
                if neighbor.0 >= grid.len() || neighbor.1 >= grid[0].len() {
                    continue;
                }
                if !grid[neighbor.0][neighbor.1] {
                    continue;
                }

                let tentative_g_score = g_score.get(&current).unwrap_or(&i32::MAX) + 10;

                if tentative_g_score < *g_score.get(&neighbor).unwrap_or(&i32::MAX) {
                    came_from.insert(neighbor, current);
                    g_score.insert(neighbor, tentative_g_score);
                    open_set.push(Node {
                        pos: neighbor,
                        f_score: tentative_g_score + heuristic(neighbor, goal),
                    });
                }
            }
        }

        None
    }

    /// Benchmark decision trees
    async fn benchmark_decision_trees(&self) -> Result<BenchmarkResult> {
        let mut result = BenchmarkResult::new(
            "AI Decision Trees".to_string(),
            BenchmarkCategory::Ai,
        );

        result.add_metadata("description".to_string(),
            "Tests decision tree evaluation for unit AI".to_string());

        #[derive(Clone)]
        struct UnitState {
            health: f32,
            ammo: i32,
            enemy_distance: f32,
            allies_nearby: usize,
        }

        #[derive(Clone, Copy, PartialEq)]
        enum Decision {
            Attack,
            Retreat,
            Defend,
            Regroup,
            Patrol,
        }

        fn evaluate_decision(state: &UnitState) -> Decision {
            if state.health < 30.0 {
                Decision::Retreat
            } else if state.ammo == 0 {
                Decision::Retreat
            } else if state.enemy_distance < 50.0 && state.allies_nearby >= 3 {
                Decision::Attack
            } else if state.enemy_distance < 100.0 {
                if state.allies_nearby >= 2 {
                    Decision::Attack
                } else {
                    Decision::Regroup
                }
            } else if state.enemy_distance < 300.0 {
                Decision::Defend
            } else {
                Decision::Patrol
            }
        }

        let states: Vec<UnitState> = (0..1000)
            .map(|i| UnitState {
                health: ((i * 17) % 100) as f32,
                ammo: ((i * 31) % 50) as i32,
                enemy_distance: ((i * 47) % 500) as f32,
                allies_nearby: ((i * 13) % 10),
            })
            .collect();

        // Measure
        for _ in 0..self.config.measurement_iterations {
            let start = Instant::now();
            for state in &states {
                let _ = evaluate_decision(state);
            }
            let duration = start.elapsed();

            result.add_measurement(Measurement::new(
                duration.as_micros() as f64,
                MeasurementUnit::Microseconds,
            ));
        }

        Ok(result)
    }

    /// Benchmark state machines
    async fn benchmark_state_machines(&self) -> Result<BenchmarkResult> {
        let mut result = BenchmarkResult::new(
            "AI State Machines".to_string(),
            BenchmarkCategory::Ai,
        );

        result.add_metadata("description".to_string(),
            "Tests FSM update performance for unit behaviors".to_string());

        #[derive(Clone, Copy, PartialEq)]
        enum State {
            Idle,
            Moving,
            Attacking,
            Retreating,
            Dead,
        }

        struct StateMachine {
            current_state: State,
            timer: f32,
        }

        impl StateMachine {
            fn update(&mut self, dt: f32, health: f32, enemy_nearby: bool) -> State {
                self.timer += dt;

                self.current_state = match self.current_state {
                    State::Idle => {
                        if health <= 0.0 {
                            State::Dead
                        } else if enemy_nearby {
                            State::Attacking
                        } else if self.timer > 5.0 {
                            State::Moving
                        } else {
                            State::Idle
                        }
                    }
                    State::Moving => {
                        if health <= 0.0 {
                            State::Dead
                        } else if enemy_nearby {
                            State::Attacking
                        } else if self.timer > 10.0 {
                            State::Idle
                        } else {
                            State::Moving
                        }
                    }
                    State::Attacking => {
                        if health <= 0.0 {
                            State::Dead
                        } else if health < 30.0 {
                            State::Retreating
                        } else if !enemy_nearby {
                            State::Idle
                        } else {
                            State::Attacking
                        }
                    }
                    State::Retreating => {
                        if health <= 0.0 {
                            State::Dead
                        } else if health > 70.0 {
                            State::Idle
                        } else {
                            State::Retreating
                        }
                    }
                    State::Dead => State::Dead,
                };

                self.current_state
            }
        }

        let mut machines: Vec<StateMachine> = (0..1000)
            .map(|_| StateMachine {
                current_state: State::Idle,
                timer: 0.0,
            })
            .collect();

        // Measure
        for _ in 0..self.config.measurement_iterations {
            let start = Instant::now();
            for (i, machine) in machines.iter_mut().enumerate() {
                let health = ((i * 17) % 100) as f32;
                let enemy_nearby = (i % 3) == 0;
                machine.update(0.016, health, enemy_nearby);
            }
            let duration = start.elapsed();

            result.add_measurement(Measurement::new(
                duration.as_micros() as f64,
                MeasurementUnit::Microseconds,
            ));
        }

        Ok(result)
    }

    /// Benchmark behavior trees
    async fn benchmark_behavior_trees(&self) -> Result<BenchmarkResult> {
        let mut result = BenchmarkResult::new(
            "AI Behavior Trees".to_string(),
            BenchmarkCategory::Ai,
        );

        result.add_metadata("description".to_string(),
            "Tests behavior tree execution for complex AI".to_string());

        #[derive(Clone, Copy, PartialEq)]
        enum BTStatus {
            Success,
            Failure,
            Running,
        }

        struct BTContext {
            health: f32,
            ammo: i32,
            enemy_visible: bool,
            target_in_range: bool,
        }

        fn run_behavior_tree(ctx: &BTContext) -> BTStatus {
            // Simplified behavior tree: Check health -> Check ammo -> Engage or Retreat
            if ctx.health < 30.0 {
                return BTStatus::Failure; // Retreat
            }

            if ctx.ammo == 0 {
                return BTStatus::Failure; // Need to resupply
            }

            if ctx.enemy_visible {
                if ctx.target_in_range {
                    BTStatus::Success // Engage
                } else {
                    BTStatus::Running // Move to range
                }
            } else {
                BTStatus::Running // Search
            }
        }

        let contexts: Vec<BTContext> = (0..1000)
            .map(|i| BTContext {
                health: ((i * 17) % 100) as f32,
                ammo: ((i * 31) % 50) as i32,
                enemy_visible: (i % 3) == 0,
                target_in_range: (i % 5) == 0,
            })
            .collect();

        // Measure
        for _ in 0..self.config.measurement_iterations {
            let start = Instant::now();
            for ctx in &contexts {
                let _ = run_behavior_tree(ctx);
            }
            let duration = start.elapsed();

            result.add_measurement(Measurement::new(
                duration.as_micros() as f64,
                MeasurementUnit::Microseconds,
            ));
        }

        Ok(result)
    }

    /// Benchmark target selection
    async fn benchmark_target_selection(&self) -> Result<BenchmarkResult> {
        let mut result = BenchmarkResult::new(
            "AI Target Selection".to_string(),
            BenchmarkCategory::Ai,
        );

        result.add_metadata("description".to_string(),
            "Tests enemy targeting algorithm performance".to_string());

        #[derive(Clone)]
        struct Target {
            distance: f32,
            health: f32,
            threat_level: f32,
        }

        fn select_best_target(targets: &[Target]) -> Option<usize> {
            if targets.is_empty() {
                return None;
            }

            let mut best_idx = 0;
            let mut best_score = f32::MIN;

            for (i, target) in targets.iter().enumerate() {
                // Score based on threat, proximity, and vulnerability
                let score = target.threat_level * 0.5
                    + (1.0 / (target.distance + 1.0)) * 0.3
                    + (100.0 - target.health) * 0.002;

                if score > best_score {
                    best_score = score;
                    best_idx = i;
                }
            }

            Some(best_idx)
        }

        let targets: Vec<Target> = (0..100)
            .map(|i| Target {
                distance: ((i * 17) % 500) as f32,
                health: ((i * 31) % 100) as f32,
                threat_level: ((i * 47) % 10) as f32,
            })
            .collect();

        // Measure
        for _ in 0..self.config.measurement_iterations {
            let start = Instant::now();
            for _ in 0..1000 {
                let _ = select_best_target(&targets);
            }
            let duration = start.elapsed();

            result.add_measurement(Measurement::new(
                duration.as_micros() as f64,
                MeasurementUnit::Microseconds,
            ));
        }

        Ok(result)
    }

    /// Benchmark formation logic
    async fn benchmark_formation_logic(&self) -> Result<BenchmarkResult> {
        let mut result = BenchmarkResult::new(
            "AI Formation Logic".to_string(),
            BenchmarkCategory::Ai,
        );

        result.add_metadata("description".to_string(),
            "Tests unit group formation and coordination".to_string());

        #[derive(Clone)]
        struct Unit {
            position: [f32; 2],
            assigned_position: [f32; 2],
        }

        fn update_formation(units: &mut [Unit], center: [f32; 2], spacing: f32) {
            let rows = (units.len() as f32).sqrt().ceil() as usize;

            for (i, unit) in units.iter_mut().enumerate() {
                let row = i / rows;
                let col = i % rows;

                unit.assigned_position = [
                    center[0] + (col as f32 - rows as f32 / 2.0) * spacing,
                    center[1] + (row as f32 - rows as f32 / 2.0) * spacing,
                ];
            }
        }

        let mut units: Vec<Unit> = (0..100)
            .map(|i| Unit {
                position: [i as f32 * 5.0, (i / 10) as f32 * 5.0],
                assigned_position: [0.0, 0.0],
            })
            .collect();

        // Measure
        for _ in 0..self.config.measurement_iterations {
            let start = Instant::now();
            for _ in 0..100 {
                update_formation(&mut units, [500.0, 500.0], 10.0);
            }
            let duration = start.elapsed();

            result.add_measurement(Measurement::new(
                duration.as_micros() as f64,
                MeasurementUnit::Microseconds,
            ));
        }

        Ok(result)
    }

    /// Benchmark tactical analysis
    async fn benchmark_tactical_analysis(&self) -> Result<BenchmarkResult> {
        let mut result = BenchmarkResult::new(
            "AI Tactical Analysis".to_string(),
            BenchmarkCategory::Ai,
        );

        result.add_metadata("description".to_string(),
            "Tests strategic map analysis and planning".to_string());

        #[derive(Clone)]
        struct TacticalPoint {
            position: [f32; 2],
            control_value: f32,
            threat_level: f32,
        }

        fn analyze_tactical_situation(points: &[TacticalPoint]) -> Vec<usize> {
            let mut priorities: Vec<(usize, f32)> = points
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let score = p.control_value - p.threat_level * 0.5;
                    (i, score)
                })
                .collect();

            priorities.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

            priorities.into_iter().map(|(i, _)| i).take(10).collect()
        }

        let points: Vec<TacticalPoint> = (0..200)
            .map(|i| TacticalPoint {
                position: [(i % 20) as f32 * 50.0, (i / 20) as f32 * 50.0],
                control_value: ((i * 17) % 100) as f32,
                threat_level: ((i * 31) % 100) as f32,
            })
            .collect();

        // Measure
        for _ in 0..self.config.measurement_iterations.min(50) {
            let start = Instant::now();
            let _ = analyze_tactical_situation(&points);
            let duration = start.elapsed();

            result.add_measurement(Measurement::new(
                duration.as_micros() as f64,
                MeasurementUnit::Microseconds,
            ));
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ai_benchmarks_run() {
        let config = BenchmarkConfig {
            warmup_iterations: 2,
            measurement_iterations: 5,
            ..Default::default()
        };

        let mut ai_bench = AiBenchmarks::new(&config);
        let results = ai_bench.run_all().await.unwrap();

        assert!(!results.is_empty(), "Should have AI benchmark results");

        for result in results {
            assert!(!result.measurements.is_empty(), "Each benchmark should have measurements");
            assert_eq!(result.category, BenchmarkCategory::Ai);
        }
    }

    #[test]
    fn test_pathfinding() {
        let grid = vec![vec![true; 20]; 20];
        let path = AiBenchmarks::astar_pathfind(&grid, (0, 0), (19, 19));

        assert!(path.is_some(), "Should find path in open grid");
        assert_eq!(path.as_ref().unwrap().first(), Some(&(0, 0)));
        assert_eq!(path.as_ref().unwrap().last(), Some(&(19, 19)));
    }
}