//! Ordinary Differential Equation (ODE) solver system.
//!
//! This module provides various numerical integration methods for solving systems
//! of ordinary differential equations. It includes implementations of:
//! - Euler's method (first-order)
//! - Midpoint method (second-order Runge-Kutta)
//! - Classic fourth-order Runge-Kutta method
//! - Fifth-order Runge-Kutta method
//!
//! The system is designed around traits that allow different ODE systems
//! to be integrated using the same numerical methods.

/// A state vector for storing the state variables of an ODE system.
///
/// This is essentially a dynamically-sized array of floating-point values
/// that represents the current state of a system of differential equations.
pub type StateVector = Vec<f32>;

/// Trait for systems of ordinary differential equations.
///
/// Any system that implements this trait can be integrated using the
/// numerical methods provided by the `IntegrationSystem`.
pub trait ODESystem {
    /// Get the current state of the system.
    ///
    /// # Returns
    /// A vector containing the current values of all state variables
    fn get_state(&self) -> StateVector;

    /// Set the state of the system.
    ///
    /// # Arguments
    /// * `state` - The new state vector
    /// * `start_index` - Index to start reading from (for multi-system integration)
    ///
    /// # Returns
    /// The next index after this system's state variables
    fn set_state(&mut self, state: &StateVector, start_index: usize) -> usize;

    /// Compute the derivatives of the state variables.
    ///
    /// This is the core function that defines the differential equations.
    /// Given the current time and state, it computes the rate of change
    /// for each state variable.
    ///
    /// # Arguments
    /// * `t` - Current time
    /// * `test_state` - Optional test state for evaluation (use current state if None)
    /// * `start_index` - Index to start reading from the test state
    ///
    /// # Returns
    /// Tuple of (derivatives vector, next index)
    fn compute_derivatives(
        &self,
        t: f32,
        test_state: Option<&StateVector>,
        start_index: usize,
    ) -> (StateVector, usize);
}

/// Integration system providing various numerical methods for ODE solving.
pub struct IntegrationSystem;

impl IntegrationSystem {
    /// Integrate using Euler's method (first-order).
    ///
    /// This is the simplest but least accurate method. It requires only
    /// a single evaluation of the derivatives per timestep.
    ///
    /// # Arguments
    /// * `system` - The ODE system to integrate
    /// * `dt` - Time step size
    pub fn euler_integrate<T: ODESystem>(system: &mut T, dt: f32) {
        // Get current state
        let y0 = system.get_state();

        // Compute derivatives at current state
        let (dydt, _) = system.compute_derivatives(0.0, None, 0);

        // Euler step: y1 = y0 + dt * dydt
        let y1: StateVector = y0
            .iter()
            .zip(dydt.iter())
            .map(|(y, dy)| y + dt * dy)
            .collect();

        // Update system state
        system.set_state(&y1, 0);
    }

    /// Integrate using the midpoint method (second-order Runge-Kutta).
    ///
    /// This method evaluates derivatives at two points and provides
    /// better accuracy than Euler's method.
    ///
    /// # Arguments
    /// * `system` - The ODE system to integrate
    /// * `dt` - Time step size
    pub fn midpoint_integrate<T: ODESystem>(system: &mut T, dt: f32) {
        // Get current state
        let y0 = system.get_state();

        // First evaluation at current state
        let (dydt, _) = system.compute_derivatives(0.0, None, 0);

        // Compute midpoint state
        let ymid: StateVector = y0
            .iter()
            .zip(dydt.iter())
            .map(|(y, dy)| y + dt * dy / 2.0)
            .collect();

        // Second evaluation at midpoint
        let (dydt_mid, _) = system.compute_derivatives(dt / 2.0, Some(&ymid), 0);

        // Final step using midpoint derivatives
        let y1: StateVector = y0
            .iter()
            .zip(dydt_mid.iter())
            .map(|(y, dy)| y + dt * dy)
            .collect();

        // Update system state
        system.set_state(&y1, 0);
    }

    /// Integrate using fourth-order Runge-Kutta method.
    ///
    /// This is the classic RK4 method that requires four evaluations
    /// of the derivatives per timestep but provides excellent accuracy.
    ///
    /// # Arguments
    /// * `system` - The ODE system to integrate
    /// * `dt` - Time step size
    pub fn runge_kutta_integrate<T: ODESystem>(system: &mut T, dt: f32) {
        let dt2 = dt / 2.0;
        let dt6 = dt / 6.0;

        // Get current state
        let y0 = system.get_state();

        // First step: evaluate at current state
        let (k1, _) = system.compute_derivatives(0.0, None, 0);

        // Second step: evaluate at y0 + dt/2 * k1
        let yt: StateVector = y0.iter().zip(k1.iter()).map(|(y, k)| y + dt2 * k).collect();
        let (k2, _) = system.compute_derivatives(dt2, Some(&yt), 0);

        // Third step: evaluate at y0 + dt/2 * k2
        let yt: StateVector = y0.iter().zip(k2.iter()).map(|(y, k)| y + dt2 * k).collect();
        let (k3, _) = system.compute_derivatives(dt2, Some(&yt), 0);

        // Fourth step: evaluate at y0 + dt * k3
        let yt: StateVector = y0.iter().zip(k3.iter()).map(|(y, k)| y + dt * k).collect();
        let (k4, _) = system.compute_derivatives(dt, Some(&yt), 0);

        // Combine results: y1 = y0 + dt/6 * (k1 + 2*k2 + 2*k3 + k4)
        let y1: StateVector = y0
            .iter()
            .zip(k1.iter())
            .zip(k2.iter())
            .zip(k3.iter())
            .zip(k4.iter())
            .map(|((((y, k1), k2), k3), k4)| y + dt6 * (k1 + 2.0 * k2 + 2.0 * k3 + k4))
            .collect();

        // Update system state
        system.set_state(&y1, 0);
    }

    /// Integrate using fifth-order Runge-Kutta method.
    ///
    /// This method provides higher accuracy than RK4 but requires
    /// six evaluations of the derivatives per timestep.
    ///
    /// # Arguments
    /// * `system` - The ODE system to integrate
    /// * `dt` - Time step size
    pub fn runge_kutta5_integrate<T: ODESystem>(system: &mut T, dt: f32) {
        // Runge-Kutta-Fehlberg coefficients
        const A2: f32 = 0.2;
        const A3: f32 = 0.3;
        const A4: f32 = 0.6;
        const A5: f32 = 1.0;
        const A6: f32 = 0.875;

        const B21: f32 = 0.2;
        const B31: f32 = 3.0 / 40.0;
        const B32: f32 = 9.0 / 40.0;
        const B41: f32 = 0.3;
        const B42: f32 = -0.9;
        const B43: f32 = 1.2;
        const B51: f32 = -11.0 / 54.0;
        const B52: f32 = 2.5;
        const B53: f32 = -70.0 / 27.0;
        const B54: f32 = 35.0 / 27.0;
        const B61: f32 = 1631.0 / 55296.0;
        const B62: f32 = 175.0 / 512.0;
        const B63: f32 = 575.0 / 13824.0;
        const B64: f32 = 44275.0 / 110592.0;
        const B65: f32 = 253.0 / 4096.0;

        const C1: f32 = 37.0 / 378.0;
        const C3: f32 = 250.0 / 621.0;
        const C4: f32 = 125.0 / 594.0;
        const C6: f32 = 512.0 / 1771.0;

        // Get current state
        let y0 = system.get_state();

        // First step
        let (k1, _) = system.compute_derivatives(0.0, None, 0);
        let ytmp: StateVector = y0
            .iter()
            .zip(k1.iter())
            .map(|(y, k)| y + B21 * dt * k)
            .collect();

        // Second step
        let (k2, _) = system.compute_derivatives(A2 * dt, Some(&ytmp), 0);
        let ytmp: StateVector = y0
            .iter()
            .zip(k1.iter())
            .zip(k2.iter())
            .map(|((y, k1), k2)| y + dt * (B31 * k1 + B32 * k2))
            .collect();

        // Third step
        let (k3, _) = system.compute_derivatives(A3 * dt, Some(&ytmp), 0);
        let ytmp: StateVector = y0
            .iter()
            .zip(k1.iter())
            .zip(k2.iter())
            .zip(k3.iter())
            .map(|(((y, k1), k2), k3)| y + dt * (B41 * k1 + B42 * k2 + B43 * k3))
            .collect();

        // Fourth step
        let (k4, _) = system.compute_derivatives(A4 * dt, Some(&ytmp), 0);
        let ytmp: StateVector = y0
            .iter()
            .zip(k1.iter())
            .zip(k2.iter())
            .zip(k3.iter())
            .zip(k4.iter())
            .map(|((((y, k1), k2), k3), k4)| y + dt * (B51 * k1 + B52 * k2 + B53 * k3 + B54 * k4))
            .collect();

        // Fifth step
        let (k5, _) = system.compute_derivatives(A5 * dt, Some(&ytmp), 0);
        let ytmp: StateVector = y0
            .iter()
            .zip(k1.iter())
            .zip(k2.iter())
            .zip(k3.iter())
            .zip(k4.iter())
            .zip(k5.iter())
            .map(|(((((y, k1), k2), k3), k4), k5)| {
                y + dt * (B61 * k1 + B62 * k2 + B63 * k3 + B64 * k4 + B65 * k5)
            })
            .collect();

        // Sixth step
        let (k6, _) = system.compute_derivatives(A6 * dt, Some(&ytmp), 0);

        // Final result
        let y1: StateVector = y0
            .iter()
            .zip(k1.iter())
            .zip(k3.iter())
            .zip(k4.iter())
            .zip(k6.iter())
            .map(|((((y, k1), k3), k4), k6)| y + dt * (C1 * k1 + C3 * k3 + C4 * k4 + C6 * k6))
            .collect();

        // Update system state
        system.set_state(&y1, 0);
    }
}

/// Example implementation of a simple harmonic oscillator for testing.
///
/// This represents the second-order ODE: x'' + omega^2 * x = 0
/// Which is converted to the first-order system:
/// x' = v
/// v' = -omega^2 * x
#[derive(Debug, Clone)]
pub struct HarmonicOscillator {
    /// Position
    pub x: f32,
    /// Velocity  
    pub v: f32,
    /// Angular frequency
    pub omega: f32,
}

impl HarmonicOscillator {
    /// Create a new harmonic oscillator.
    ///
    /// # Arguments
    /// * `x0` - Initial position
    /// * `v0` - Initial velocity
    /// * `omega` - Angular frequency
    pub fn new(x0: f32, v0: f32, omega: f32) -> Self {
        Self {
            x: x0,
            v: v0,
            omega,
        }
    }

    /// Get the total energy of the oscillator (should be conserved).
    pub fn energy(&self) -> f32 {
        0.5 * (self.v * self.v + self.omega * self.omega * self.x * self.x)
    }
}

impl ODESystem for HarmonicOscillator {
    fn get_state(&self) -> StateVector {
        vec![self.x, self.v]
    }

    fn set_state(&mut self, state: &StateVector, start_index: usize) -> usize {
        self.x = state[start_index];
        self.v = state[start_index + 1];
        start_index + 2
    }

    fn compute_derivatives(
        &self,
        _t: f32,
        test_state: Option<&StateVector>,
        start_index: usize,
    ) -> (StateVector, usize) {
        let (x, v) = if let Some(state) = test_state {
            (state[start_index], state[start_index + 1])
        } else {
            (self.x, self.v)
        };

        // x' = v
        // v' = -omega^2 * x
        let derivatives = vec![v, -self.omega * self.omega * x];
        (derivatives, start_index + 2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_approx_eq(a: f32, b: f32, tolerance: f32) {
        assert!(
            (a - b).abs() < tolerance,
            "Expected {} ≈ {}, difference: {}",
            a,
            b,
            (a - b).abs()
        );
    }

    #[test]
    fn test_harmonic_oscillator_energy_conservation_euler() {
        let mut oscillator = HarmonicOscillator::new(1.0, 0.0, 1.0);
        let initial_energy = oscillator.energy();
        let dt = 0.001;
        let steps = 1000;

        for _ in 0..steps {
            IntegrationSystem::euler_integrate(&mut oscillator, dt);
        }

        // Euler method doesn't conserve energy well, but it shouldn't explode
        let final_energy = oscillator.energy();
        assert!(final_energy > 0.0 && final_energy < 10.0 * initial_energy);
    }

    #[test]
    fn test_harmonic_oscillator_energy_conservation_rk4() {
        let mut oscillator = HarmonicOscillator::new(1.0, 0.0, 1.0);
        let initial_energy = oscillator.energy();
        let dt = 0.01;
        let steps = 1000;

        for _ in 0..steps {
            IntegrationSystem::runge_kutta_integrate(&mut oscillator, dt);
        }

        // RK4 should conserve energy much better
        let final_energy = oscillator.energy();
        assert_approx_eq(final_energy, initial_energy, 0.01);
    }

    #[test]
    fn test_harmonic_oscillator_period() {
        let mut oscillator = HarmonicOscillator::new(1.0, 0.0, 1.0);
        let dt = 0.001;
        let expected_period = 2.0 * std::f32::consts::PI; // T = 2π/ω for ω = 1

        // Integrate for one full period
        let steps = (expected_period / dt) as usize;
        for _ in 0..steps {
            IntegrationSystem::runge_kutta_integrate(&mut oscillator, dt);
        }

        // Should be back near the starting position
        assert_approx_eq(oscillator.x, 1.0, 0.01);
        assert_approx_eq(oscillator.v, 0.0, 0.1);
    }

    #[test]
    fn test_all_integration_methods_basic() {
        // Test that all methods can integrate a simple system without crashing
        let dt = 0.001;

        let mut osc1 = HarmonicOscillator::new(1.0, 0.0, 1.0);
        let mut osc2 = HarmonicOscillator::new(1.0, 0.0, 1.0);
        let mut osc3 = HarmonicOscillator::new(1.0, 0.0, 1.0);
        let mut osc4 = HarmonicOscillator::new(1.0, 0.0, 1.0);

        for _ in 0..100 {
            IntegrationSystem::euler_integrate(&mut osc1, dt);
            IntegrationSystem::midpoint_integrate(&mut osc2, dt);
            IntegrationSystem::runge_kutta_integrate(&mut osc3, dt);
            IntegrationSystem::runge_kutta5_integrate(&mut osc4, dt);
        }

        // All should have moved from initial position
        assert!(osc1.x != 1.0);
        assert!(osc2.x != 1.0);
        assert!(osc3.x != 1.0);
        assert!(osc4.x != 1.0);

        // All should still have finite values
        assert!(osc1.x.is_finite() && osc1.v.is_finite());
        assert!(osc2.x.is_finite() && osc2.v.is_finite());
        assert!(osc3.x.is_finite() && osc3.v.is_finite());
        assert!(osc4.x.is_finite() && osc4.v.is_finite());
    }

    #[test]
    fn test_state_vector_operations() {
        let oscillator = HarmonicOscillator::new(2.0, 3.0, 1.5);

        let state = oscillator.get_state();
        assert_eq!(state, vec![2.0, 3.0]);

        let mut new_oscillator = HarmonicOscillator::new(0.0, 0.0, 1.0);
        let next_index = new_oscillator.set_state(&vec![5.0, -2.0], 0);

        assert_eq!(new_oscillator.x, 5.0);
        assert_eq!(new_oscillator.v, -2.0);
        assert_eq!(next_index, 2);
    }

    #[test]
    fn test_derivatives_computation() {
        let oscillator = HarmonicOscillator::new(2.0, 3.0, 1.5);

        let (derivatives, next_index) = oscillator.compute_derivatives(0.0, None, 0);

        // x' = v = 3.0
        // v' = -omega^2 * x = -1.5^2 * 2.0 = -4.5
        assert_eq!(derivatives, vec![3.0, -4.5]);
        assert_eq!(next_index, 2);

        // Test with test state
        let test_state = vec![1.0, -1.0];
        let (derivatives, _) = oscillator.compute_derivatives(0.0, Some(&test_state), 0);

        // x' = v = -1.0
        // v' = -omega^2 * x = -1.5^2 * 1.0 = -2.25
        assert_eq!(derivatives, vec![-1.0, -2.25]);
    }
}
