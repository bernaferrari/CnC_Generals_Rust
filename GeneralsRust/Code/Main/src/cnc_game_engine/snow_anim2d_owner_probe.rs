//! Actual CnCGameEngine clock regression. Winit constructors run on the main thread.
use super::*;

pub(super) fn run() -> anyhow::Result<()> {
    let event_loop = EventLoop::new()?;
    let window = Arc::new(
        event_loop.create_window(
            WindowAttributes::default()
                .with_title("snow and Anim2D clock ownership regression")
                .with_visible(false),
        )?,
    );
    let args = Arc::new(CommandLineArgs::parse_from_args(vec![
        "snow-anim2d-owner-probe".to_string(),
        "--runtime_host=headless".to_string(),
        "--noaudio".to_string(),
    ])?);
    let mut first = pollster::block_on(CnCGameEngine::new(Arc::clone(&window), Arc::clone(&args)))?;
    first.current_state = GameState::Menu;
    // A future fixed observation also remains after any incidental constructor
    // clock observation. All later assertions use this exact injected Instant.
    let start = Instant::now() + Duration::from_secs(60);
    let step = game_engine::common::game_common::SECONDS_PER_LOGICFRAME_REAL;
    assert_eq!(first.snow_anim2d_dt_for_present_with_now(|| start), step);

    // Use the actual constructor while the first engine owns a live timestamp.
    // No second Engine/GameClient/Object surrogate is installed in a registry.
    let mut second = pollster::block_on(CnCGameEngine::new(window, args))?;
    second.current_state = GameState::Menu;
    assert_eq!(first.game_logic.get_frame(), 0);
    assert_eq!(second.game_logic.get_frame(), 0);
    assert_eq!(
        first.snow_anim2d_dt_for_present_with_now(|| start + Duration::from_millis(1)),
        0.0,
        "constructing another engine must preserve the first clock",
    );
    assert_eq!(
        second.snow_anim2d_dt_for_present_with_now(|| start),
        step,
        "a fresh engine must receive its own first client-only tick",
    );
    println!("PASS snow clock: two actual constructors / independent first tick");

    assert_eq!(
        first.snow_anim2d_dt_for_present_with_now(|| start + Duration::from_millis(32)),
        0.0
    );
    assert_eq!(
        first.snow_anim2d_dt_for_present_with_now(|| start + Duration::from_millis(34)),
        step
    );
    assert_eq!(
        second.snow_anim2d_dt_for_present_with_now(|| start + Duration::from_millis(34)),
        step
    );
    assert_eq!(
        first.snow_anim2d_dt_for_present_with_now(|| start + Duration::from_millis(50)),
        0.0
    );
    assert_eq!(
        first.snow_anim2d_dt_for_present_with_now(|| start + Duration::from_millis(68)),
        step,
        "rejected observations must not move the accepted timestamp",
    );
    assert_eq!(
        second.snow_anim2d_dt_for_present_with_now(|| start + Duration::from_secs(2)),
        step,
        "a long gap is capped to one client-only tick, not catch-up steps",
    );
    assert_eq!(
        second.snow_anim2d_dt_for_present_with_now(|| start + Duration::from_millis(2001)),
        0.0
    );
    assert_eq!(
        first.snow_anim2d_dt_for_present_with_now(|| start + Duration::from_millis(60)),
        0.0
    );
    assert_eq!(
        first.snow_anim2d_dt_for_present_with_now(|| start + Duration::from_millis(102)),
        step
    );
    println!("PASS snow clock: interleaving / cap / rejected and backward observations");

    first.current_state = GameState::InGame;
    first.host_match_logic_frame = Some(100);
    first.host_snow_logic_frame_applied = None;
    assert_eq!(
        first.snow_anim2d_dt_for_present_with_now(|| panic!("InGame must not observe wall time")),
        step
    );
    assert_eq!(
        first.snow_anim2d_dt_for_present_with_now(|| panic!(
            "same logic frame must not observe wall time"
        )),
        0.0
    );
    first.host_match_logic_frame = Some(103);
    assert_eq!(
        first.snow_anim2d_dt_for_present_with_now(|| panic!(
            "logic catch-up must not observe wall time"
        )),
        3.0 * step
    );
    first.host_match_logic_frame = Some(50);
    assert_eq!(
        first.snow_anim2d_dt_for_present_with_now(|| panic!(
            "logic rewind must not observe wall time"
        )),
        0.0
    );
    assert_eq!(first.host_snow_logic_frame_applied, Some(50));
    first.host_match_logic_frame = None;
    assert_eq!(
        first.snow_anim2d_dt_for_present_with_now(|| panic!(
            "missing match frame must not observe wall time"
        )),
        0.0
    );
    assert_eq!(first.host_snow_logic_frame_applied, Some(50));
    first.current_state = GameState::Paused;
    first.host_match_logic_frame = Some(51);
    assert_eq!(
        first.snow_anim2d_dt_for_present_with_now(|| panic!(
            "Paused must retain the logic-frame branch"
        )),
        step
    );
    println!("PASS snow clock: InGame / Paused / unchanged frame / catch-up / rewind gates");

    // Actual engine reset clears the match stamp, while the client-only clock
    // retains the same lifetime as its client/engine. It must not reset B.
    first.host_reset_game_logic();
    assert_eq!(first.host_snow_logic_frame_applied, None);
    first.current_state = GameState::Menu;
    assert_eq!(
        first.snow_anim2d_dt_for_present_with_now(|| start + Duration::from_millis(103)),
        0.0
    );
    assert_eq!(
        first.snow_anim2d_dt_for_present_with_now(|| start + Duration::from_millis(136)),
        step
    );
    assert_eq!(
        second.snow_anim2d_dt_for_present_with_now(|| start + Duration::from_millis(2001)),
        0.0
    );
    assert_eq!(
        second.snow_anim2d_dt_for_present_with_now(|| start + Duration::from_millis(2034)),
        step
    );
    first.current_state = GameState::InGame;
    first.host_match_logic_frame = Some(7);
    assert_eq!(
        first.snow_anim2d_dt_for_present_with_now(|| panic!(
            "post-reset logic frame must not use wall time"
        )),
        step
    );
    assert_eq!(first.game_logic.get_frame(), 0);
    assert_eq!(second.game_logic.get_frame(), 0);
    println!("PASS snow clock: actual match reset / first logic tick / no simulation advancement");

    drop(second);
    first.current_state = GameState::Menu;
    assert_eq!(
        first.snow_anim2d_dt_for_present_with_now(|| start + Duration::from_millis(170)),
        step
    );
    println!("PASS snow clock: second engine destruction leaves first clock intact");
    println!("snow_anim2d_owner_probe: 5 clock case groups passed");
    Ok(())
}
