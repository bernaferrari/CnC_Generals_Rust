use crate::debug_debug::Debug;
use crate::debug_io::{DebugIOInterface, StringType};

pub struct DebugIOOds;

impl DebugIOOds {
    pub fn create() -> Box<dyn DebugIOInterface> {
        Box::new(Self)
    }
}

impl DebugIOInterface for DebugIOOds {
    fn read(&mut self, _buf: &mut [u8]) -> usize {
        0
    }

    fn write(&mut self, kind: StringType, src: Option<&str>, message: Option<&str>) {
        let Some(message) = message else {
            return;
        };

        if kind == StringType::StructuredCmdReply {
            return;
        }

        let mut full = String::new();
        if let Some(src) = src {
            full.push_str(src);
            full.push_str(": ");
        }
        full.push_str(message);

        #[cfg(windows)]
        unsafe {
            use std::ffi::CString;
            use windows_sys::Win32::System::Diagnostics::Debug::OutputDebugStringA;

            if let Ok(cstr) = CString::new(full) {
                OutputDebugStringA(cstr.as_ptr());
            }
        }

        #[cfg(not(windows))]
        {
            eprintln!("{full}");
        }
    }

    fn emergency_flush(&mut self) {}

    fn execute(&mut self, _dbg: &mut Debug, _cmd: &str, _structured: bool, _argv: &[&str]) {}

    fn delete(self: Box<Self>) {}
}
