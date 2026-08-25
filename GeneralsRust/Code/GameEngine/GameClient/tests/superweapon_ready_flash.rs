//! C++ InGameUI.cpp:3654-3666 READY strip flash (not static yellow 0:00).

use game_client_rust::gui::ingame_ui::{
    SUPERWEAPON_FLASH_RGBA, SUPERWEAPON_NORMAL_POINT_SIZE, SUPERWEAPON_NORMAL_RGBA,
    SUPERWEAPON_READY_POINT_SIZE, SuperweaponFlashState, superweapon_ready_draw_style,
};

#[test]
fn ready_strip_flashes_instead_of_static_yellow() {
    // Given: C++ flash duration 15 frames, usedFlashColor starts true.
    let mut state = SuperweaponFlashState {
        used_flash_color: true,
        last_flash_frame: 0,
    };

    // When: countdown is not ready
    let (idle, idle_size) =
        superweapon_ready_draw_style(1, false, 15, SUPERWEAPON_FLASH_RGBA, &mut state);
    // Then: default white, normal font
    assert_eq!(idle, SUPERWEAPON_NORMAL_RGBA);
    assert_eq!(idle_size, SUPERWEAPON_NORMAL_POINT_SIZE);

    // When: READY at frame 1 (toggle window)
    let (ready0, ready_size) =
        superweapon_ready_draw_style(1, true, 15, SUPERWEAPON_FLASH_RGBA, &mut state);
    // Then: ready font, default color (not static yellow)
    assert_eq!(ready_size, SUPERWEAPON_READY_POINT_SIZE);
    assert_eq!(ready0, SUPERWEAPON_NORMAL_RGBA);
    assert_ne!(ready0, SUPERWEAPON_FLASH_RGBA);

    // When: READY after flash duration
    let (ready1, _) =
        superweapon_ready_draw_style(16, true, 15, SUPERWEAPON_FLASH_RGBA, &mut state);
    // Then: flash color (blink)
    assert_eq!(ready1, SUPERWEAPON_FLASH_RGBA);

    // When: READY after another duration
    let (ready2, _) =
        superweapon_ready_draw_style(31, true, 15, SUPERWEAPON_FLASH_RGBA, &mut state);
    // Then: back to default
    assert_eq!(ready2, SUPERWEAPON_NORMAL_RGBA);
}
