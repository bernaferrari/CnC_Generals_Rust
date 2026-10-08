//! Smudge.cpp: used sets drain from the head, free sets admit from the head.
use super::*;

fn add_set(manager: &mut SmudgeManager, size: f32) -> SmudgeSetHandle {
    let set = manager.add_smudge_set();
    set.lock().unwrap().add_smudge_to_set().size = size;
    set
}

#[test]
fn reset_recycles_smudges_in_cpp_used_set_order() {
    let mut manager = SmudgeManager::new();
    let first = add_set(&mut manager, 1.0);
    let second = add_set(&mut manager, 2.0);
    manager.reset();

    let reused_first = manager.add_smudge_set();
    assert!(Arc::ptr_eq(&first, &reused_first));
    assert_eq!(reused_first.lock().unwrap().add_smudge_to_set().size, 2.0);
    let reused_second = manager.add_smudge_set();
    assert!(Arc::ptr_eq(&second, &reused_second));
    assert_eq!(reused_second.lock().unwrap().add_smudge_to_set().size, 1.0);
}

#[test]
fn reset_appends_sets_after_existing_free_head() {
    let mut manager = SmudgeManager::new();
    let removed = add_set(&mut manager, 1.0);
    let remaining = add_set(&mut manager, 2.0);
    manager.remove_smudge_set(&removed);
    manager.reset();

    let reused = manager.add_smudge_set();
    assert!(Arc::ptr_eq(&removed, &reused));
    assert_eq!(reused.lock().unwrap().used_smudges()[0].size, 1.0);
    let reused_remaining = manager.add_smudge_set();
    assert!(Arc::ptr_eq(&remaining, &reused_remaining));
    assert_eq!(reused_remaining.lock().unwrap().used_smudge_count(), 0);
}

#[test]
fn removal_preserves_surviving_set_and_decal_order() {
    let mut manager = SmudgeManager::new();
    let removed = add_set(&mut manager, 1.0);
    let _ = add_set(&mut manager, 2.0);
    let _ = add_set(&mut manager, 3.0);
    manager.remove_smudge_set(&removed);

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
