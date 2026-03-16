# GPUI Integration Guide

This document describes the GPUI integration with the Command & Conquer Generals Rust port.

## Overview

GPUI (a modern Rust UI framework) has been integrated with the existing W3D-based UI system to enable a gradual migration path to modern, performant UI components.

## Architecture

### Components

1. **GPUI Integration Layer** (`ui/gpui_integration.rs`)
   - Manages GPUI application context
   - Handles GPUI window lifecycle
   - Provides asset caching (textures, fonts)
   - Bridges events between GPUI and main UI systems

2. **Hybrid UI System** (`ui/hybrid_ui.rs`)
   - Coordinates W3D and GPUI rendering
   - Routes screens to appropriate UI system
   - Handles smooth transitions between systems
   - Monitors performance metrics

3. **GPUI Components** (`ui/gpui_main_menu.rs`)
   - Modern, animated UI components
   - Event handling and state management
   - Smooth animations and transitions

## Screen Routing

Screens can be configured to use either W3D or GPUI:

```rust
// Use GPUI for a screen
hybrid_ui.register_gpui_screen(Screen::MainMenu);

// Check which system a screen uses
if hybrid_ui.should_use_gpui(Screen::MainMenu) {
    // Screen uses GPUI
} else {
    // Screen uses W3D
}
```

### Default Configuration

- **GPUI Screens**: MainMenu, Options, Credits
- **W3D Screens**: GameHUD (performance-critical)

## Usage

### Initializing the Hybrid System

```rust
use generals_main::ui::hybrid_ui::HybridUISystem;

// Create and initialize
let mut hybrid_ui = HybridUISystem::new();
hybrid_ui.initialize(1600, 900)?;

// Register GPUI screens
hybrid_ui.register_gpui_screen(Screen::MainMenu);
```

### Navigation

```rust
// Navigate to any screen
hybrid_ui.navigate_to(Screen::MainMenu, cx)?;

// The system automatically routes to GPUI or W3D
// based on screen configuration
```

### Render Modes

```rust
use generals_main::ui::hybrid_ui::RenderMode;

// W3D only (legacy mode)
hybrid_ui.set_render_mode(RenderMode::W3D);

// GPUI only (all screens use GPUI)
hybrid_ui.set_render_mode(RenderMode::GPUI);

// Hybrid (automatic routing - recommended)
hybrid_ui.set_render_mode(RenderMode::Hybrid);
```

## Game Loop Integration

```rust
// In your main game loop:

// 1. Handle input
for event in input_events {
    hybrid_ui.handle_input(&event, cx)?;
}

// 2. Update UI
hybrid_ui.update(delta_time, cx)?;

// 3. Render UI
hybrid_ui.render(&mut render_context, cx)?;
```

## Performance Monitoring

```rust
let metrics = hybrid_ui.performance();
println!("W3D render time: {:.2}ms", metrics.w3d_render_time);
println!("GPUI render time: {:.2}ms", metrics.gpui_render_time);
println!("Total render time: {:.2}ms", metrics.total_render_time);
println!("FPS: {:.1}", metrics.fps);
```

## Gradual Migration Path

### Phase 1: Non-Critical Screens
- Main Menu ✓
- Options Menu ✓
- Credits Screen ✓

### Phase 2: Setup Screens
- Skirmish Setup
- Campaign Menu
- Faction Selection
- Map Selection

### Phase 3: Dialogs
- Save/Load Dialogs
- Confirmation Dialogs
- Message Boxes

### Phase 4: In-Game UI (Optional)
- HUD elements (careful with performance)
- Control Bar
- Minimap

## Benefits of GPUI

1. **Modern Animations**: Smooth 60fps animations
2. **Better Layout**: Flexible, responsive layouts
3. **Type Safety**: Compile-time UI validation
4. **Performance**: GPU-accelerated rendering
5. **Maintainability**: Declarative UI code
6. **Accessibility**: Better accessibility support

## Backward Compatibility

The hybrid system maintains full backward compatibility:

- W3D UI continues to work unchanged
- Screens can be migrated incrementally
- No breaking changes to existing code
- Performance is not degraded

## Running the Demo

```bash
# Run the GPUI integration demo
cargo run --bin gpui_demo

# Run the main game with GPUI integration
cargo run --bin generals
```

## Troubleshooting

### GPUI Windows Not Showing

Ensure GPUI is properly initialized:
```rust
hybrid_ui.initialize(width, height)?;
```

### Screens Not Using GPUI

Check screen registration:
```rust
hybrid_ui.register_gpui_screen(Screen::YourScreen);
```

### Performance Issues

Use W3D for performance-critical screens:
```rust
// Don't register GameHUD for GPUI
// It will automatically use W3D
```

## Future Enhancements

- [ ] Complete GPUI migration of all screens
- [ ] Custom GPU shaders for UI effects
- [ ] Touch/gesture support
- [ ] Accessibility improvements
- [ ] Internationalization support
- [ ] Theming system
- [ ] UI editor tools

## Contributing

When adding new UI screens, consider implementing them in GPUI first for better maintainability and modern features.

## Resources

- [GPUI Documentation](https://github.com/zed-industries/zed/tree/main/crates/gpui)
- [W3D UI Reference](../GameEngine/GameClient/gui/)
- [UI System Guide](./README.md)
