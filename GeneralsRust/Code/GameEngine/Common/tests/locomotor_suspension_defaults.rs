//! Bitwise constructor defaults and raw parseReal values through both real loaders.
#[path = "support/locomotor_suspension.rs"]
mod fixture;

#[test]
fn omitted_defaults() {
    fixture::check("CommonOmitted", "omitted", fixture::common);
}
#[test]
fn authored_signed_zeros_and_mixed_omissions() {
    fixture::check("CommonZero", "zero", fixture::common);
}
#[test]
fn distinct_nonzero_and_unclamped_controls() {
    fixture::check("CommonControl", "control", fixture::common);
}
