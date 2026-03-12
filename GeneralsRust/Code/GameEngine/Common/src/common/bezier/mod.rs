// Bezier curve modules
pub mod bez_fwd_iterator;
pub mod bezier_segment;

// Re-export main types for convenience
pub use bez_fwd_iterator::BezFwdIterator;
pub use bezier_segment::{BezierSegment, Bool, Coord3D, Int, Matrix4x4, Real, VecCoord3D, Vector4};
