//! Callback signatures equivalent to C++ `WindowProc.h`.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogMessage {
    InitDialog,
    Command { control_id: u16, notify_code: u16 },
    Close,
}

pub trait WindowProcedures {
    fn image_packer_proc(&mut self, message: DialogMessage) -> bool;
    fn image_error_proc(&mut self, message: DialogMessage) -> bool;
    fn page_error_proc(&mut self, message: DialogMessage) -> bool;
    fn directory_select_proc(&mut self, message: DialogMessage) -> bool;
    fn preview_proc(&mut self, message: DialogMessage) -> bool;
}

#[derive(Default)]
pub struct NullWindowProcedures;

impl WindowProcedures for NullWindowProcedures {
    fn image_packer_proc(&mut self, _message: DialogMessage) -> bool {
        false
    }

    fn image_error_proc(&mut self, _message: DialogMessage) -> bool {
        false
    }

    fn page_error_proc(&mut self, _message: DialogMessage) -> bool {
        false
    }

    fn directory_select_proc(&mut self, _message: DialogMessage) -> bool {
        false
    }

    fn preview_proc(&mut self, _message: DialogMessage) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::{DialogMessage, NullWindowProcedures, WindowProcedures};

    #[test]
    fn null_impl_returns_false() {
        let mut hooks = NullWindowProcedures;
        assert!(!hooks.image_packer_proc(DialogMessage::InitDialog));
    }
}
