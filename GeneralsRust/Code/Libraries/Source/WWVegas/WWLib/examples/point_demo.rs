use wwlib_rust::point::{Point2D, Point3D, TPoint2D, TPoint3D};

fn main() {
    println!("=== Point2D Examples ===");

    // Create points
    let p1 = Point2D::new(3, 4);
    let p2 = Point2D::new(1, 2);

    println!("Point 1: {}", p1);
    println!("Point 2: {}", p2);

    // Basic arithmetic
    println!("Addition: {} + {} = {}", p1, p2, p1 + p2);
    println!("Subtraction: {} - {} = {}", p1, p2, p1 - p2);
    println!("Scalar multiplication: {} * 2 = {}", p1, p1 * 2);
    println!("Negation: -{} = {}", p1, -p1);

    // Vector operations with floats
    let pf1 = TPoint2D::new(3.0, 4.0);
    let pf2 = TPoint2D::new(1.0, 2.0);

    println!("Length of {}: {:.3}", pf1, pf1.length());
    println!(
        "Distance from {} to {}: {:.3}",
        pf1,
        pf2,
        pf1.distance_to(pf2)
    );
    println!("Normalized {}: {}", pf1, pf1.normalize());
    println!("Dot product {}.dot({}): {}", pf1, pf2, pf1.dot_product(pf2));
    println!(
        "Cross product {}.cross({}): {}",
        pf1,
        pf2,
        pf1.cross_product(pf2)
    );

    println!("\n=== Point3D Examples ===");

    // 3D operations
    let p3d1 = Point3D::new(1, 2, 3);
    let p3d2 = Point3D::new(4, 5, 6);

    println!("3D Point 1: {}", p3d1);
    println!("3D Point 2: {}", p3d2);
    println!("3D Addition: {} + {} = {}", p3d1, p3d2, p3d1 + p3d2);
    println!("3D Subtraction: {} - {} = {}", p3d2, p3d1, p3d2 - p3d1);

    // 3D vector operations with floats
    let p3df1 = TPoint3D::new(1.0, 0.0, 0.0);
    let p3df2 = TPoint3D::new(0.0, 1.0, 0.0);

    println!("Length of {}: {:.3}", p3df1, p3df1.length());
    println!(
        "3D Cross product {} × {} = {}",
        p3df1,
        p3df2,
        p3df1.cross_product(p3df2)
    );

    // Conversions
    let p2d_from_3d: Point2D = p3d1.into();
    println!("Convert 3D {} to 2D: {}", p3d1, p2d_from_3d);

    let p3d_from_2d: Point3D = p1.into();
    println!("Convert 2D {} to 3D: {}", p1, p3d_from_2d);

    // Mixed 2D/3D operations
    let p3d_mixed = p3d1 + p1;
    println!("3D + 2D: {} + {} = {}", p3d1, p1, p3d_mixed);

    println!("\n=== Mathematical Verification ===");

    // Verify mathematical properties
    let unit_x = TPoint3D::new(1.0, 0.0, 0.0);
    let unit_y = TPoint3D::new(0.0, 1.0, 0.0);
    let unit_z = TPoint3D::new(0.0, 0.0, 1.0);

    println!("Unit vectors:");
    println!("X: {}, Y: {}, Z: {}", unit_x, unit_y, unit_z);

    // Standard cross product identities
    println!("X × Y = {}", unit_x.cross_product(unit_y));
    println!("Y × Z = {}", unit_y.cross_product(unit_z));
    println!("Z × X = {}", unit_z.cross_product(unit_x));

    // Verify normalization
    let vec = TPoint3D::new(3.0, 4.0, 12.0);
    let normalized = vec.normalize();
    println!("Original vector: {} (length: {:.3})", vec, vec.length());
    println!(
        "Normalized: {} (length: {:.3})",
        normalized,
        normalized.length()
    );
}
