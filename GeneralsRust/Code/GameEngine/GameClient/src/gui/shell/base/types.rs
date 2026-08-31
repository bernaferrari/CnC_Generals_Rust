// Split from `gui/shell/base.rs` dump. Included by `base/mod.rs`.
// C++ Shell.h: window rect/layout state primitives, WindowLayout contract.
/// Shell system errors
#[derive(Error, Debug)]
pub enum ShellError {
    #[error("Layout not found: {0}")]
    LayoutNotFound(String),
    #[error("Shell stack overflow - maximum {max} screens reached")]
    StackOverflow { max: usize },
    #[error("Cannot pop from empty shell stack")]
    EmptyStack,
    #[error("Shell not initialized")]
    NotInitialized,
    #[error("Layout operation failed: {0}")]
    LayoutError(String),
    #[error("Animation error: {0}")]
    AnimationError(String),
}

/// Animation types for window transitions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationType {
    /// No animation
    None,
    /// Slide from left
    SlideLeft,
    /// Slide from right
    SlideRight,
    /// Slide from top
    SlideTop,
    /// Slide from bottom
    SlideBottom,
    /// Slide from right (fast)
    SlideRightFast,
    /// Slide from top (fast)
    SlideTopFast,
    /// Slide from bottom (timed)
    SlideBottomTimed,
    /// Spiral animation
    Spiral,
}

/// Window position and size
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl WindowRect {
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn zero() -> Self {
        Self::new(0, 0, 0, 0)
    }
}

/// Window layout state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutState {
    /// Layout is being initialized
    Initializing,
    /// Layout is active and visible
    Active,
    /// Layout is shutting down
    ShuttingDown,
    /// Layout is hidden but still on stack
    Hidden,
    /// Layout is being destroyed
    Destroying,
}

/// Represents a window layout/screen in the shell system
pub trait WindowLayout {
    /// Get the filename this layout was loaded from
    fn get_filename(&self) -> &str;

    /// Initialize the layout - called when pushed or when becoming top of stack
    fn run_init(&mut self, data: Option<&dyn std::any::Any>) -> Result<(), ShellError>;

    /// Update the layout - called every frame for all layouts on stack
    fn run_update(&mut self, data: Option<&dyn std::any::Any>) -> Result<(), ShellError>;

    /// Shutdown the layout - called when being popped or when new layout pushed on top
    /// The immediate_pop parameter indicates if the layout should shutdown immediately
    fn run_shutdown(&mut self, immediate_pop: &mut bool) -> Result<(), ShellError>;

    /// Show/hide the layout
    fn hide(&mut self, hide: bool);

    /// Check if the layout is hidden
    fn is_hidden(&self) -> bool;

    /// Bring the layout to the front
    fn bring_forward(&mut self);

    /// Destroy all windows in this layout
    fn destroy_windows(&mut self);

    /// Mark the first window as an image-backed shell background when applicable.
    fn set_first_window_image(&mut self) {}

    /// Get the current state of the layout
    fn get_state(&self) -> LayoutState;

    /// Set the layout state
    fn set_state(&mut self, state: LayoutState);
}

/// Default implementation of WindowLayout for basic functionality
#[derive(Debug)]
pub struct BasicWindowLayout {
    filename: String,
    state: LayoutState,
    hidden: bool,
    bounds: WindowRect,
    created_at: Instant,
    layout: Option<Rc<RefCell<ManagerWindowLayout>>>,
}

impl BasicWindowLayout {
    pub fn new(filename: String) -> Self {
        Self {
            filename,
            state: LayoutState::Initializing,
            hidden: true,
            bounds: WindowRect::zero(),
            created_at: Instant::now(),
            layout: None,
        }
    }

    fn ensure_layout(&mut self) -> Result<Rc<RefCell<ManagerWindowLayout>>, ShellError> {
        if let Some(layout) = &self.layout {
            return Ok(layout.clone());
        }

        let (layout, _info) =
            with_window_manager(|manager| manager.create_layout_with_windows(&self.filename))
                .map_err(|err| ShellError::LayoutError(format!("{}: {:?}", self.filename, err)))?;

        self.layout = Some(layout.clone());
        Ok(layout)
    }
}

impl WindowLayout for BasicWindowLayout {
    fn get_filename(&self) -> &str {
        &self.filename
    }

    fn run_init(&mut self, data: Option<&dyn std::any::Any>) -> Result<(), ShellError> {
        log::debug!("Initializing layout: {}", self.filename);

        let layout = self.ensure_layout()?;
        {
            let layout_ref = layout.borrow();
            layout_ref.run_init(data);
        }

        self.state = LayoutState::Active;
        self.hidden = false;
        Ok(())
    }

    fn run_update(&mut self, data: Option<&dyn std::any::Any>) -> Result<(), ShellError> {
        if let Some(layout) = &self.layout {
            layout.borrow().run_update(data);
        }
        Ok(())
    }

    fn run_shutdown(&mut self, immediate_pop: &mut bool) -> Result<(), ShellError> {
        log::debug!(
            "Shutting down layout: {} (immediate: {})",
            self.filename,
            *immediate_pop
        );

        if let Some(layout) = &self.layout {
            let layout_ref = layout.borrow();
            layout_ref.run_shutdown(Some(immediate_pop as &mut dyn std::any::Any));
        }

        if *immediate_pop {
            self.state = LayoutState::Destroying;
        } else {
            self.state = LayoutState::ShuttingDown;
        }

        Ok(())
    }

    fn hide(&mut self, hide: bool) {
        if let Some(layout) = &self.layout {
            layout.borrow().hide(hide);
        }
        self.hidden = hide;
    }

    fn is_hidden(&self) -> bool {
        self.layout
            .as_ref()
            .map(|layout| layout.borrow().is_hidden())
            .unwrap_or(self.hidden)
    }

    fn bring_forward(&mut self) {
        log::debug!("Bringing layout to front: {}", self.filename);
        if let Some(layout) = &self.layout {
            layout.borrow_mut().bring_forward();
        }
    }

    fn set_first_window_image(&mut self) {
        if let Some(layout) = &self.layout {
            if let Some(first_window) = layout.borrow().get_first_window() {
                first_window.borrow_mut().set_status(WindowStatus::IMAGE);
            }
        }
    }

    fn destroy_windows(&mut self) {
        log::debug!("Destroying windows for layout: {}", self.filename);
        if let Some(layout) = self.layout.take() {
            with_window_manager(|manager| manager.destroy_layout(&layout));
        }
        self.state = LayoutState::Destroying;
        self.hidden = true;
    }

    fn get_state(&self) -> LayoutState {
        self.state
    }

    fn set_state(&mut self, state: LayoutState) {
        self.state = state;
    }
}

/// 2D coordinate structure
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Coord2D {
    pub x: i32,
    pub y: i32,
}

impl Coord2D {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    pub fn zero() -> Self {
        Self::new(0, 0)
    }
}

/// 2D float coordinate for animation velocities.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coord2DF {
    pub x: f32,
    pub y: f32,
}

impl Coord2DF {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// Color representation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn black() -> Self {
        Self::new(0, 0, 0, 255)
    }

    pub fn white() -> Self {
        Self::new(255, 255, 255, 255)
    }

    pub fn transparent() -> Self {
        Self::new(0, 0, 0, 0)
    }
}
