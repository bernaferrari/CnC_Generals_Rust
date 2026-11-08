
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DebugType {
    Information,
    Warning,
    Error,
    User,
}

pub type PrintFunc = fn(DebugType, &str);
pub type AssertPrintFunc = fn(&str);
pub type TriggerFunc = fn(i32) -> bool;
pub type ProfileFunc = fn(&str);

pub(crate) struct DebugHandlers {
    pub(crate) message: Option<PrintFunc>,
    pub(crate) assert: Option<AssertPrintFunc>,
    pub(crate) trigger: Option<TriggerFunc>,
    pub(crate) profile_start: Option<ProfileFunc>,
    pub(crate) profile_stop: Option<ProfileFunc>,
}

impl DebugHandlers {
    pub(crate) fn new() -> Self {
        Self {
            message: None,
            assert: None,
            trigger: None,
            profile_start: None,
            profile_stop: None,
        }
    }
}



#[allow(non_snake_case)]
pub fn Convert_System_Error_To_String(error_id: i32, buffer: &mut [u8]) {
    if buffer.is_empty() {
        return;
    }
    let msg = std::io::Error::from_raw_os_error(error_id).to_string();
    let bytes = msg.as_bytes();
    let len = bytes.len().min(buffer.len().saturating_sub(1));
    buffer[..len].copy_from_slice(&bytes[..len]);
    buffer[len] = 0;
}

#[allow(non_snake_case)]
pub fn Get_Last_System_Error() -> i32 {
    std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
}

#[allow(non_snake_case)]
pub fn WWDebug_Install_Message_Handler(session: &crate::WwDebugSession, func: Option<PrintFunc>) -> Option<PrintFunc> {
    let mut handlers = session.handlers.lock().expect("debug handler lock poisoned");
    let previous = handlers.message;
    handlers.message = func;
    previous
}

#[allow(non_snake_case)]
pub fn WWDebug_Install_Assert_Handler(session: &crate::WwDebugSession, func: Option<AssertPrintFunc>) -> Option<AssertPrintFunc> {
    let mut handlers = session.handlers.lock().expect("debug handler lock poisoned");
    let previous = handlers.assert;
    handlers.assert = func;
    previous
}

#[allow(non_snake_case)]
pub fn WWDebug_Install_Trigger_Handler(session: &crate::WwDebugSession, func: Option<TriggerFunc>) -> Option<TriggerFunc> {
    let mut handlers = session.handlers.lock().expect("debug handler lock poisoned");
    let previous = handlers.trigger;
    handlers.trigger = func;
    previous
}

#[allow(non_snake_case)]
pub fn WWDebug_Install_Profile_Start_Handler(session: &crate::WwDebugSession, func: Option<ProfileFunc>) -> Option<ProfileFunc> {
    let mut handlers = session.handlers.lock().expect("debug handler lock poisoned");
    let previous = handlers.profile_start;
    handlers.profile_start = func;
    previous
}

#[allow(non_snake_case)]
pub fn WWDebug_Install_Profile_Stop_Handler(session: &crate::WwDebugSession, func: Option<ProfileFunc>) -> Option<ProfileFunc> {
    let mut handlers = session.handlers.lock().expect("debug handler lock poisoned");
    let previous = handlers.profile_stop;
    handlers.profile_stop = func;
    previous
}

fn dispatch_message(session: &crate::WwDebugSession, kind: DebugType, message: &str) {
    let handlers = session.handlers.lock().expect("debug handler lock poisoned");
    if let Some(handler) = handlers.message {
        handler(kind, message);
    }
}

#[allow(non_snake_case)]
pub fn WWDebug_Printf(session: &crate::WwDebugSession, message: &str) {
    dispatch_message(session, DebugType::Information, message);
}

#[allow(non_snake_case)]
pub fn WWDebug_Printf_Warning(session: &crate::WwDebugSession, message: &str) {
    dispatch_message(session, DebugType::Warning, message);
}

#[allow(non_snake_case)]
pub fn WWDebug_Printf_Error(session: &crate::WwDebugSession, message: &str) {
    dispatch_message(session, DebugType::Error, message);
}

#[macro_export]
macro_rules! wwdebug_printf {
    ($($arg:tt)*) => {
        $crate::wwdebug::WWDebug_Printf(&format!($($arg)*))
    };
}

#[macro_export]
macro_rules! wwdebug_printf_warning {
    ($($arg:tt)*) => {
        $crate::wwdebug::WWDebug_Printf_Warning(&format!($($arg)*))
    };
}

#[macro_export]
macro_rules! wwdebug_printf_error {
    ($($arg:tt)*) => {
        $crate::wwdebug::WWDebug_Printf_Error(&format!($($arg)*))
    };
}

#[allow(non_snake_case)]
pub fn WWDebug_Assert_Fail(session: &crate::WwDebugSession, expr: &str, file: &str, line: i32) {
    let message = format!("{file} ({line}) Assert: {expr}\n");
    let handlers = session.handlers.lock().expect("debug handler lock poisoned");
    if let Some(assert_handler) = handlers.assert {
        assert_handler(&message);
        return;
    }

    drop(handlers);
    if cfg!(debug_assertions) {
        panic!("{message}");
    }
}

#[allow(non_snake_case)]
pub fn WWDebug_Assert_Fail_Print(session: &crate::WwDebugSession, expr: &str, file: &str, line: i32, string: &str) {
    let message = format!("{file} ({line}) Assert: {expr} {string}\n");
    let handlers = session.handlers.lock().expect("debug handler lock poisoned");
    if let Some(assert_handler) = handlers.assert {
        assert_handler(&message);
        return;
    }

    drop(handlers);
    if cfg!(debug_assertions) {
        panic!("{message}");
    }
}

#[allow(non_snake_case)]
pub fn WWDebug_Check_Trigger(session: &crate::WwDebugSession, trigger_num: i32) -> bool {
    let handlers = session.handlers.lock().expect("debug handler lock poisoned");
    if let Some(trigger_handler) = handlers.trigger {
        return trigger_handler(trigger_num);
    }
    false
}

#[allow(non_snake_case)]
pub fn WWDebug_Profile_Start(session: &crate::WwDebugSession, title: &str) {
    let handlers = session.handlers.lock().expect("debug handler lock poisoned");
    if let Some(handler) = handlers.profile_start {
        handler(title);
    }
}

#[allow(non_snake_case)]
pub fn WWDebug_Profile_Stop(session: &crate::WwDebugSession, title: &str) {
    let handlers = session.handlers.lock().expect("debug handler lock poisoned");
    if let Some(handler) = handlers.profile_stop {
        handler(title);
    }
}

#[allow(non_snake_case)]
/// Writes at most 507 message bytes to the 512-byte DBWIN view, followed by NUL.
/// Longer messages are truncated by byte; messages containing NUL are sent empty.
pub fn WWDebug_DBWin32_Message_Handler(message: &str) {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE, WAIT_OBJECT_0};
        use windows_sys::Win32::Storage::FileSystem::SYNCHRONIZE;
        use windows_sys::Win32::System::Diagnostics::Debug::OutputDebugStringA;
        use windows_sys::Win32::System::Memory::{
            CreateFileMappingA, FILE_MAP_WRITE, MapViewOfFile, PAGE_READWRITE, UnmapViewOfFile,
        };
        use windows_sys::Win32::System::Threading::{
            EVENT_MODIFY_STATE, INFINITE, OpenEventA, SetEvent, WaitForSingleObject,
        };

        const VIEW_SIZE: usize = 512;
        const HEADER_SIZE: usize = std::mem::size_of::<u32>();

        let ready_event =
            // SAFETY: The event name is static and NUL-terminated.
            unsafe { OpenEventA(SYNCHRONIZE, 0, b"DBWIN_BUFFER_READY\0".as_ptr()) };
        if ready_event.is_null() {
            // SAFETY: [Category 8 — FFI] opens the well-known DBWIN event by static NUL-terminated name.
            unsafe { OutputDebugStringA(b"DBWIN_BUFFER_READY missing\0".as_ptr()) };
            return;
        }
        let data_event =
            // SAFETY: [Category 8 — FFI] opens the well-known DBWIN event by static NUL-terminated name.
            unsafe { OpenEventA(EVENT_MODIFY_STATE, 0, b"DBWIN_DATA_READY\0".as_ptr()) };
        if data_event.is_null() {
            // SAFETY: [Category 8 — FFI] opens the well-known DBWIN event by static NUL-terminated name.
            unsafe { CloseHandle(ready_event) };
            return;
        }

        // SAFETY: [Category 8 — FFI] creates the standard 4096-byte DBWIN shared section with a static name.
        let mapping = unsafe {
            CreateFileMappingA(
                INVALID_HANDLE_VALUE,
                std::ptr::null(),
                PAGE_READWRITE,
                0,
                4096,
                b"DBWIN_BUFFER\0".as_ptr(),
            )
        };
        if mapping.is_null() {
            // SAFETY: [Category 8 — FFI] closes a handle previously opened/mapped above, exactly once per branch.
            unsafe {
                CloseHandle(ready_event);
                CloseHandle(data_event);
            }
            return;
        }

        // SAFETY: mapping is an open mapping handle and the requested view fits the section.
        let view = unsafe { MapViewOfFile(mapping, FILE_MAP_WRITE, 0, 0, VIEW_SIZE) };
        if view.Value.is_null() {
            // SAFETY: [Category 8 — FFI] closes a handle previously opened/mapped above, exactly once per branch.
            unsafe {
                CloseHandle(mapping);
                CloseHandle(ready_event);
                CloseHandle(data_event);
            }
            return;
        }

        // SAFETY: ready_event is an open event with SYNCHRONIZE access.
        if unsafe { WaitForSingleObject(ready_event, INFINITE) } == WAIT_OBJECT_0 {
            let bytes = message.as_bytes();
            let payload = if bytes.contains(&0) { &[][..] } else { bytes };
            let len = payload.len().min(VIEW_SIZE - HEADER_SIZE - 1);
            // SAFETY: The successful wait grants access to the 512-byte writable view.
            // The header and at most 507 payload bytes plus a terminator fit in it.
            unsafe {
                let buffer = view.Value.cast::<u8>();
                buffer.cast::<u32>().write_unaligned(0);
                std::ptr::copy(payload.as_ptr(), buffer.add(HEADER_SIZE), len);
                buffer.add(HEADER_SIZE + len).write(0);
                SetEvent(data_event);
            }
        }
        // SAFETY: The view and handles were acquired successfully and are released once.
        unsafe {
            UnmapViewOfFile(view);
            CloseHandle(mapping);
            CloseHandle(data_event);
            CloseHandle(ready_event);
        }
    }

    #[cfg(not(windows))]
    {
        let _ = message;
    }
}
