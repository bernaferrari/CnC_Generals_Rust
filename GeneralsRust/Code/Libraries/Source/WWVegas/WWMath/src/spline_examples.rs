//! Comprehensive examples and tests for all spline types
//! 
//! This module demonstrates the usage and capabilities of the converted spline system

#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn test_all_spline_types_basic_usage() {
        // Test Linear Curve
        let mut linear = LinearCurve3D::new();
        linear.add_key(Vector3::new(0.0, 0.0, 0.0), 0.0);
        linear.add_key(Vector3::new(10.0, 10.0, 10.0), 1.0);
        
        let linear_result = linear.evaluate(0.5);
        assert_eq!(linear_result, Vector3::new(5.0, 5.0, 5.0));
        println!("Linear curve: {:?}", linear_result);

        // Test Hermite Spline
        let mut hermite = HermiteSpline3D::new();
        hermite.add_key(Vector3::new(0.0, 0.0, 0.0), 0.0);
        hermite.add_key(Vector3::new(10.0, 10.0, 10.0), 1.0);
        hermite.set_tangents(0, Vector3::ZERO, Vector3::new(5.0, 5.0, 5.0));
        hermite.set_tangents(1, Vector3::new(5.0, 5.0, 5.0), Vector3::ZERO);
        
        let hermite_result = hermite.evaluate(0.5);
        println!("Hermite spline: {:?}", hermite_result);
        assert!(hermite_result.is_valid());

        // Test Cardinal Spline
        let mut cardinal = CardinalSpline3D::new();
        cardinal.add_key(Vector3::new(0.0, 0.0, 0.0), 0.0);
        cardinal.add_key(Vector3::new(5.0, 10.0, 5.0), 0.5);
        cardinal.add_key(Vector3::new(10.0, 0.0, 10.0), 1.0);
        cardinal.set_tightness(1, 0.5);
        
        let cardinal_result = cardinal.evaluate(0.25);
        println!("Cardinal spline: {:?}", cardinal_result);
        assert!(cardinal_result.is_valid());

        // Test Catmull-Rom Spline
        let mut catmull_rom = CatmullRomSpline3D::new();
        catmull_rom.add_key(Vector3::new(0.0, 0.0, 0.0), 0.0);
        catmull_rom.add_key(Vector3::new(5.0, 10.0, 5.0), 0.33);
        catmull_rom.add_key(Vector3::new(15.0, 10.0, 15.0), 0.67);
        catmull_rom.add_key(Vector3::new(20.0, 0.0, 20.0), 1.0);
        
        let catmull_rom_result = catmull_rom.evaluate(0.5);
        println!("Catmull-Rom spline: {:?}", catmull_rom_result);
        assert!(catmull_rom_result.is_valid());

        // Test TCB Spline
        let mut tcb = TcbSpline3D::new();
        tcb.add_key(Vector3::new(0.0, 0.0, 0.0), 0.0);
        tcb.add_key(Vector3::new(10.0, 10.0, 10.0), 0.5);
        tcb.add_key(Vector3::new(20.0, 0.0, 20.0), 1.0);
        tcb.set_tcb_params(1, 0.5, 0.25, -0.25); // Tension, Continuity, Bias
        
        let tcb_result = tcb.evaluate(0.25);
        println!("TCB spline: {:?}", tcb_result);
        assert!(tcb_result.is_valid());

        // Test Vehicle Curve
        let mut vehicle = VehicleCurve::new_with_radius(5.0);
        vehicle.add_key(Vector3::new(0.0, 0.0, 0.0), 0.0);
        vehicle.add_key(Vector3::new(10.0, 0.0, 0.0), 0.5);
        vehicle.add_key(Vector3::new(10.0, 10.0, 0.0), 1.0);
        
        let vehicle_result = vehicle.evaluate(0.75);
        println!("Vehicle curve: {:?}", vehicle_result);
        assert!(vehicle_result.is_valid());

        let (sharpness, _) = vehicle.get_current_sharpness();
        println!("Vehicle curve sharpness: {}", sharpness);
        assert!(sharpness >= 0.0 && sharpness <= 1.0);
    }

    #[test]
    fn test_1d_splines() {
        // Test Linear 1D
        let mut linear_1d = LinearCurve1D::new();
        linear_1d.add_key(0.0, 0.0, 0);
        linear_1d.add_key(100.0, 1.0, 0);
        
        let result = linear_1d.evaluate(0.5);
        assert_eq!(result, 50.0);

        // Test Hermite 1D
        let mut hermite_1d = HermiteSpline1D::new();
        hermite_1d.add_key(0.0, 0.0, 0);
        hermite_1d.add_key(100.0, 1.0, 0);
        hermite_1d.set_tangents(0, 0.0, 50.0);
        hermite_1d.set_tangents(1, 50.0, 0.0);
        
        let result = hermite_1d.evaluate(0.5);
        assert!(result != 50.0); // Should be curved, not linear

        // Test Cardinal 1D
        let mut cardinal_1d = CardinalSpline1D::new();
        cardinal_1d.add_key(0.0, 0.0, 0);
        cardinal_1d.add_key(100.0, 0.5, 0);
        cardinal_1d.add_key(0.0, 1.0, 0);
        
        let result = cardinal_1d.evaluate(0.25);
        assert!(result > 0.0); // Should be influenced by middle point

        // Test Catmull-Rom 1D
        let mut catmull_rom_1d = CatmullRomSpline1D::new();
        catmull_rom_1d.add_key(0.0, 0.0, 0);
        catmull_rom_1d.add_key(10.0, 0.25, 0);
        catmull_rom_1d.add_key(-10.0, 0.75, 0);
        catmull_rom_1d.add_key(0.0, 1.0, 0);
        
        // Test that it passes through control points
        let start = catmull_rom_1d.evaluate(0.0);
        assert_eq!(start, 0.0);
        
        let peak = catmull_rom_1d.evaluate(0.25);
        assert!((peak - 10.0).abs() < 0.01);

        // Test TCB 1D
        let mut tcb_1d = TcbSpline1D::new();
        tcb_1d.add_key(0.0, 0.0, 0);
        tcb_1d.add_key(100.0, 0.5, 0);
        tcb_1d.add_key(0.0, 1.0, 0);
        tcb_1d.set_tcb_params(1, 0.5, 0.25, -0.5);
        
        let result = tcb_1d.evaluate(0.25);
        assert!(result.is_finite());
    }

    #[test]
    fn test_spline_features() {
        let mut spline = CatmullRomSpline3D::new();
        
        // Test key management
        assert_eq!(spline.key_count(), 0);
        assert_eq!(spline.get_start_time(), 0.0);
        assert_eq!(spline.get_end_time(), 0.0);
        
        spline.add_key(Vector3::new(0.0, 0.0, 0.0), 1.0);
        spline.add_key(Vector3::new(10.0, 10.0, 10.0), 3.0);
        spline.add_key(Vector3::new(5.0, 5.0, 5.0), 2.0); // Should be inserted in middle
        
        assert_eq!(spline.key_count(), 3);
        assert_eq!(spline.get_start_time(), 1.0);
        assert_eq!(spline.get_end_time(), 3.0);
        
        // Check time-ordered insertion
        if let Some((point, time)) = spline.get_key(1) {
            assert_eq!(time, 2.0);
            assert_eq!(point, Vector3::new(5.0, 5.0, 5.0));
        }
        
        // Test looping
        assert!(!spline.is_looping());
        spline.set_looping(true);
        assert!(spline.is_looping());
        
        // Test key modification
        spline.set_key(1, Vector3::new(7.0, 7.0, 7.0));
        if let Some((point, _)) = spline.get_key(1) {
            assert_eq!(point, Vector3::new(7.0, 7.0, 7.0));
        }
        
        // Test removal
        spline.remove_key(1);
        assert_eq!(spline.key_count(), 2);
        
        // Test clear
        spline.clear_keys();
        assert_eq!(spline.key_count(), 0);
    }

    #[test] 
    fn test_spline_parameters() {
        // Test Cardinal tightness
        let mut cardinal = CardinalSpline3D::new();
        cardinal.add_key(Vector3::ZERO, 0.0);
        cardinal.set_tightness(0, 0.75);
        
        assert_eq!(cardinal.get_tightness(0), Some(0.75));
        assert_eq!(cardinal.get_tightness(1), None);

        // Test TCB parameters
        let mut tcb = TcbSpline3D::new();
        tcb.add_key(Vector3::ZERO, 0.0);
        tcb.set_tcb_params(0, 0.5, -0.25, 0.75);
        
        let params = tcb.get_tcb_params(0);
        assert!(params.is_some());
        
        let tcb_params = params.unwrap();
        assert_eq!(tcb_params.tension, 0.5);
        assert_eq!(tcb_params.continuity, -0.25);
        assert_eq!(tcb_params.bias, 0.75);
    }

    #[test]
    fn test_hermite_tangent_access() {
        let mut hermite = HermiteSpline3D::new();
        hermite.add_key(Vector3::ZERO, 0.0);
        hermite.set_tangents(0, Vector3::new(1.0, 2.0, 3.0), Vector3::new(4.0, 5.0, 6.0));
        
        let tangents = hermite.get_tangents(0);
        assert!(tangents.is_some());
        
        let (in_tan, out_tan) = tangents.unwrap();
        assert_eq!(in_tan, Vector3::new(1.0, 2.0, 3.0));
        assert_eq!(out_tan, Vector3::new(4.0, 5.0, 6.0));
    }

    #[test]
    fn test_vehicle_curve_features() {
        let mut vehicle = VehicleCurve::new();
        vehicle.initialize_arc(10.0);
        
        vehicle.add_key(Vector3::new(0.0, 0.0, 0.0), 0.0);
        vehicle.add_key(Vector3::new(20.0, 0.0, 0.0), 0.5);
        vehicle.add_key(Vector3::new(20.0, 20.0, 0.0), 1.0);
        
        // Test basic evaluation
        let result = vehicle.evaluate(0.5);
        assert!(result.is_valid());
        
        // Test sharpness tracking
        let (sharpness, pos) = vehicle.get_current_sharpness();
        assert!(sharpness >= 0.0 && sharpness <= 1.0);
        assert!(pos.is_valid());
        
        // Test last evaluation time
        let last_time = vehicle.get_last_eval_time();
        assert!(last_time >= 0.0 && last_time <= 1.0);
        
        // Test empty curve
        let mut empty_vehicle = VehicleCurve::new();
        let empty_result = empty_vehicle.evaluate(0.5);
        assert_eq!(empty_result, Vector3::ZERO);
        
        // Test single point
        let mut single_vehicle = VehicleCurve::new_with_radius(5.0);
        single_vehicle.add_key(Vector3::new(5.0, 5.0, 5.0), 0.5);
        let single_result = single_vehicle.evaluate(0.5);
        assert_eq!(single_result, Vector3::new(5.0, 5.0, 5.0));
    }

    #[test]
    fn test_curve_bounds_handling() {
        let mut spline = CatmullRomSpline3D::new();
        spline.add_key(Vector3::new(0.0, 0.0, 0.0), 1.0);
        spline.add_key(Vector3::new(10.0, 10.0, 10.0), 2.0);
        
        // Test before start time
        let before = spline.evaluate(0.5);
        assert_eq!(before, Vector3::new(0.0, 0.0, 0.0));
        
        // Test after end time
        let after = spline.evaluate(3.0);
        assert_eq!(after, Vector3::new(10.0, 10.0, 10.0));
        
        // Test exact bounds
        let start = spline.evaluate(1.0);
        assert_eq!(start, Vector3::new(0.0, 0.0, 0.0));
        
        let end = spline.evaluate(2.0);
        assert_eq!(end, Vector3::new(10.0, 10.0, 10.0));
    }

    #[test]
    fn demonstrate_spline_differences() {
        // Create the same control points for different splines
        let points = vec![
            (Vector3::new(0.0, 0.0, 0.0), 0.0),
            (Vector3::new(10.0, 5.0, 10.0), 0.33),
            (Vector3::new(5.0, 15.0, 5.0), 0.67),
            (Vector3::new(15.0, 10.0, 15.0), 1.0),
        ];
        
        let mut linear = LinearCurve3D::new();
        let mut hermite = HermiteSpline3D::new();
        let mut cardinal = CardinalSpline3D::new();
        let mut catmull_rom = CatmullRomSpline3D::new();
        let mut tcb = TcbSpline3D::new();
        
        // Add same points to all splines
        for (point, time) in &points {
            linear.add_key(*point, *time);
            hermite.add_key(*point, *time);
            cardinal.add_key(*point, *time);
            catmull_rom.add_key(*point, *time);
            tcb.add_key(*point, *time);
        }
        
        // Set some tangents for Hermite
        hermite.set_tangents(1, Vector3::new(5.0, 0.0, 5.0), Vector3::new(0.0, 10.0, 0.0));
        hermite.set_tangents(2, Vector3::new(-5.0, 5.0, -5.0), Vector3::new(10.0, -5.0, 10.0));
        
        // Set parameters for other splines
        cardinal.set_tightness(1, 0.25); // Loose
        cardinal.set_tightness(2, 0.75); // Tight
        
        tcb.set_tcb_params(1, 0.5, 0.0, 0.5);   // Moderate tension, positive bias
        tcb.set_tcb_params(2, -0.5, 0.5, -0.5); // Negative tension, positive continuity
        
        // Evaluate at the same time and show differences
        let eval_time = 0.5;
        
        let linear_result = linear.evaluate(eval_time);
        let hermite_result = hermite.evaluate(eval_time);
        let cardinal_result = cardinal.evaluate(eval_time);
        let catmull_rom_result = catmull_rom.evaluate(eval_time);
        let tcb_result = tcb.evaluate(eval_time);
        
        println!("Comparison at t=0.5:");
        println!("Linear:      {:?}", linear_result);
        println!("Hermite:     {:?}", hermite_result);
        println!("Cardinal:    {:?}", cardinal_result);
        println!("Catmull-Rom: {:?}", catmull_rom_result);
        println!("TCB:         {:?}", tcb_result);
        
        // All should be different (except possibly some coincidental matches)
        assert_ne!(linear_result, hermite_result);
        assert_ne!(linear_result, cardinal_result);
        assert_ne!(linear_result, catmull_rom_result);
        assert_ne!(linear_result, tcb_result);
        
        // All should be valid
        assert!(linear_result.is_valid());
        assert!(hermite_result.is_valid());
        assert!(cardinal_result.is_valid());
        assert!(catmull_rom_result.is_valid());
        assert!(tcb_result.is_valid());
    }
}