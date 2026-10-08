//! Smudge.cpp: used sets drain from the head, free sets admit from the head.
use super::*;

// Only API/type adapters change; original-derived values and pointer identity remain.
fn identity(set: &SmudgeSetMut<'_>) -> *const SmudgeSet {
    std::ptr::from_ref(&**set)
}

fn add_set(manager: &mut SmudgeManager, size: f32) -> *const SmudgeSet {
    let mut set = manager.add_smudge_set();
    set.add_smudge_to_set().size = size;
    identity(&set)
}

#[test]
fn reset_recycles_smudges_in_cpp_used_set_order() {
    let mut manager = SmudgeManager::new();
    let first = add_set(&mut manager, 1.0);
    let second = add_set(&mut manager, 2.0);
    manager.reset();

    let mut reused_first = manager.add_smudge_set();
    assert_eq!(first, identity(&reused_first));
    assert_eq!(reused_first.add_smudge_to_set().size, 2.0);
    let mut reused_second = manager.add_smudge_set();
    assert_eq!(second, identity(&reused_second));
    assert_eq!(reused_second.add_smudge_to_set().size, 1.0);
}

#[test]
fn reset_appends_sets_after_existing_free_head() {
    let mut manager = SmudgeManager::new();
    let removed = add_set(&mut manager, 1.0);
    let remaining = add_set(&mut manager, 2.0);
    manager.borrow_set(0).remove();
    manager.reset();

    let reused = manager.add_smudge_set();
    assert_eq!(removed, identity(&reused));
    assert_eq!(reused.used_smudges()[0].size, 1.0);
    let reused_remaining = manager.add_smudge_set();
    assert_eq!(remaining, identity(&reused_remaining));
    assert_eq!(reused_remaining.used_smudge_count(), 0);
}

#[test]
fn removal_preserves_surviving_set_and_decal_order() {
    let mut manager = SmudgeManager::new();
    let _ = add_set(&mut manager, 1.0);
    let _ = add_set(&mut manager, 2.0);
    let _ = add_set(&mut manager, 3.0);
    manager.borrow_set(0).remove();

    assert_eq!(
        manager
            .collect_used_smudges()
            .iter()
            .map(|smudge| smudge.size)
            .collect::<Vec<_>>(),
        vec![2.0, 3.0]
    );
    assert_eq!(
        manager
            .collect_decal_render_items()
            .iter()
            .map(|item| item.size)
            .collect::<Vec<_>>(),
        vec![2.0, 3.0]
    );
}
