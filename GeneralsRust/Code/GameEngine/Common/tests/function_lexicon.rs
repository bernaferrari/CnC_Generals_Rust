//! Function lexicon / FunctionPtr tests.
//!
//! `game_engine` disables lib tests (`[lib] test = false`), so these live here.

use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::function_lexicon::{
    FunctionLexicon, FunctionPtr, TableEntry, TableIndex,
};
use game_engine::common::system::subsystem_interface::SubsystemInterface;

fn test_function_1() {}
fn test_function_2() {}

fn entry(name: &str, f: fn()) -> TableEntry {
    TableEntry::new(name, Some(FunctionPtr::from_fn(f)))
}

#[test]
fn test_function_ptr_from_fn_roundtrip() {
    let ptr = FunctionPtr::from_fn(test_function_1);
    assert!(!ptr.is_null());
    let recovered = ptr.as_unit_fn().expect("from_fn should be callable");
    recovered();
    assert!(ptr.as_unit_fn().is_some());
    assert_eq!(
        FunctionPtr::from_fn(test_function_1),
        FunctionPtr::from_fn(test_function_1)
    );
    assert_ne!(
        FunctionPtr::from_fn(test_function_1),
        FunctionPtr::from_fn(test_function_2)
    );
}

#[test]
fn test_function_ptr_null_and_from_usize_zero() {
    assert!(FunctionPtr::null().is_null());
    assert!(FunctionPtr::null().as_unit_fn().is_none());
    // Safety: 0 is the documented null encoding.
    // SAFETY: 0 is the documented null encoding accepted by from_usize.
    let from_zero = unsafe { FunctionPtr::from_usize(0) };
    assert!(from_zero.is_null());
    assert!(from_zero.as_ptr().is_null());
}

#[test]
fn test_function_lexicon_lookup() {
    NameKeyGenerator::reset();
    let mut lexicon = FunctionLexicon::new();
    lexicon.load_table(
        vec![
            entry("TestFunction1", test_function_1),
            entry("TestFunction2", test_function_2),
        ],
        TableIndex::GameWinSystem,
    );
    let func1 = lexicon.find_function_by_name("TestFunction1", TableIndex::GameWinSystem);
    assert!(func1.is_some());
    func1.unwrap().as_unit_fn().expect("stored fn")();
    let func2 = lexicon.find_function_by_name("TestFunction2", TableIndex::GameWinSystem);
    assert!(func2.is_some());
}

#[test]
fn test_function_lexicon_reset_preserves_loaded_tables() {
    NameKeyGenerator::reset();
    let mut lexicon = FunctionLexicon::new();
    lexicon.load_table(
        vec![entry("TestFunction1", test_function_1)],
        TableIndex::GameWinSystem,
    );
    assert!(<FunctionLexicon as SubsystemInterface>::reset(&mut lexicon).is_ok());
    assert_eq!(lexicon.function_count(), 1);
}
