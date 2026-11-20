use math_utilities::{Vector2i, vector2i::Vector2iExt};

#[test]
fn original_signed_components_match_constructor_swap_set_and_index() {
    // Observed by executing vector2i.h's original constructors and operators.
    let mut first = Vector2i::new(i32::MIN, i32::MAX);
    let mut second = Vector2i::new(0, -37);
    let row = |first: &Vector2i, second: &Vector2i| {
        ([first[0], first[1], second[0], second[1]], first == second)
    };
    assert_eq!(row(&first, &second), ([i32::MIN, i32::MAX, 0, -37], false));
    first.swap(&mut second);
    assert_eq!(row(&first, &second), ([0, -37, i32::MIN, i32::MAX], false));
    second.set(0, 17);
    first[1] = 17;
    assert_eq!(row(&first, &second), ([0, 17, 0, 17], true));
    assert_eq!(std::mem::size_of::<Vector2i>(), 8);
    assert_eq!(std::mem::offset_of!(Vector2i, x), 0);
    assert_eq!(std::mem::offset_of!(Vector2i, y), 4);
}
