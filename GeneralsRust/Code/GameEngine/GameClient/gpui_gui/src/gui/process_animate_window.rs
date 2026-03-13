use crate::gui::source_catalog::GuiPortRecord;

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "ProcessAnimateWindow.cpp",
    "crate::gui::process_animate_window",
    "Process Animate Window",
    "Executes queued animate-window operations and applies them to legacy window trees.",
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AnimationKindPort {
    SlideFromRight,
    SlideFromLeft,
    SlideFromTop,
    SlideFromBottom,
    Spiral,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessAnimateWindowPort {
    pub kind: AnimationKindPort,
    pub delay_ms: u32,
}

impl ProcessAnimateWindowPort {
    pub fn apply(&self, start: (i32, i32), end: (i32, i32), progress: f32) -> (i32, i32) {
        let t = progress.clamp(0.0, 1.0);
        let shaped_t = match self.kind {
            AnimationKindPort::SlideFromRight
            | AnimationKindPort::SlideFromLeft
            | AnimationKindPort::SlideFromTop
            | AnimationKindPort::SlideFromBottom => t,
            AnimationKindPort::Spiral => t * t,
        };
        let x = start.0 as f32 + (end.0 - start.0) as f32 * shaped_t;
        let y = start.1 as f32 + (end.1 - start.1) as f32 * shaped_t;
        (x.round() as i32, y.round() as i32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_interpolates_between_positions() {
        let process = ProcessAnimateWindowPort {
            kind: AnimationKindPort::SlideFromRight,
            delay_ms: 0,
        };

        assert_eq!(process.apply((0, 0), (100, 50), 0.5), (50, 25));
    }
}
