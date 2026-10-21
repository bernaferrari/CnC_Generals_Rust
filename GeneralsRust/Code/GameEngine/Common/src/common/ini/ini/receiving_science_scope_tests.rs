//! Operation-local immutable Science capabilities restore on every exit.
use super::*;
#[test]
fn receiving_science_scope_restores_nested_error_and_panic_without_global_state() {
    let outer = Arc::new(crate::common::rts::science::ScienceNameFacts::default());
    let inner = Arc::new(crate::common::rts::science::ScienceNameFacts::default());
    let mut ini = INI::new();
    assert!(ini.receiving_science_names().is_none());
    let value = ini.with_receiving_science_names(outer.clone(), |ini| {
        assert!(std::ptr::eq(
            ini.receiving_science_names().unwrap(),
            outer.as_ref()
        ));
        let error: Result<(), &str> = ini.with_receiving_science_names(inner.clone(), |ini| {
            assert!(std::ptr::eq(
                ini.receiving_science_names().unwrap(),
                inner.as_ref()
            ));
            Err("actual nested parser error")
        });
        assert_eq!(error, Err("actual nested parser error"));
        assert!(std::ptr::eq(
            ini.receiving_science_names().unwrap(),
            outer.as_ref()
        ));
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ini.with_receiving_science_names(inner.clone(), |ini| {
                assert!(std::ptr::eq(
                    ini.receiving_science_names().unwrap(),
                    inner.as_ref()
                ));
                panic!("actual nested parser panic");
            })
        }));
        assert!(panic.is_err());
        assert!(std::ptr::eq(
            ini.receiving_science_names().unwrap(),
            outer.as_ref()
        ));
        17
    });
    assert_eq!(value, 17);
    assert!(ini.receiving_science_names().is_none());
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        ini.with_receiving_science_names(outer, |_| panic!("actual outer parser panic"))
    }));
    assert!(panic.is_err());
    assert!(
        ini.receiving_science_names().is_none(),
        "caught constructor panic must not leave a receiving capability on a reused INI"
    );
}
