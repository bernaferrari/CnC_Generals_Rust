use once_cell::sync::OnceCell;
use std::sync::Mutex;

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

struct DebugHandlers {
    message: Option<PrintFunc>,
    assert: Option<AssertPrintFunc>,
    trigger: Option<TriggerFunc>,
    profile_start: Option<ProfileFunc>,
    profile_stop: Option<ProfileFunc>,
}

impl DebugHandlers {
    fn new() -> Self {
        Self {
            message: None,
            assert: None,
            trigger: None,
            profile_start: None,
            profile_stop: None,
        }
    }
}

fn handlers() -> &'static Mutex<DebugHandlers> {
    static HANDLERS: OnceCell<Mutex<DebugHandlers>> = OnceCell::new();
    HANDLERS.get_or_init(|| Mutex::new(DebugHandlers::new()))
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
pub fn WWDebug_Install_Message_Handler(func: Option<PrintFunc>) -> Option<PrintFunc> {
    let mut handlers = handlers().lock().expect("debug handler lock poisoned");
    let previous = handlers.message;
    handlers.message = func;
    previous
}

#[allow(non_snake_case)]
pub fn WWDebug_Install_Assert_Handler(func: Option<AssertPrintFunc>) -> Option<AssertPrintFunc> {
    let mut handlers = handlers().lock().expect("debug handler lock poisoned");
    let previous = handlers.assert;
    handlers.assert = func;
    previous
}

#[allow(non_snake_case)]
pub fn WWDebug_Install_Trigger_Handler(func: Option<TriggerFunc>) -> Option<TriggerFunc> {
    let mut handlers = handlers().lock().expect("debug handler lock poisoned");
    let previous = handlers.trigger;
    handlers.trigger = func;
    previous
}

#[allow(non_snake_case)]
pub fn WWDebug_Install_Profile_Start_Handler(func: Option<ProfileFunc>) -> Option<ProfileFunc> {
    let mut handlers = handlers().lock().expect("debug handler lock poisoned");
    let previous = handlers.profile_start;
    handlers.profile_start = func;
    previous
}

#[allow(non_snake_case)]
pub fn WWDebug_Install_Profile_Stop_Handler(func: Option<ProfileFunc>) -> Option<ProfileFunc> {
    let mut handlers = handlers().lock().expect("debug handler lock poisoned");
    let previous = handlers.profile_stop;
    handlers.profile_stop = func;
    previous
}

fn dispatch_message(kind: DebugType, message: &str) {
    let handlers = handlers().lock().expect("debug handler lock poisoned");
    if let Some(handler) = handlers.message {
        handler(kind, message);
    }
}

#[allow(non_snake_case)]
pub fn WWDebug_Printf(message: &str) {
    dispatch_message(DebugType::Information, message);
}

#[allow(non_snake_case)]
pub fn WWDebug_Printf_Warning(message: &str) {
    dispatch_message(DebugType::Warning, message);
}

#[allow(non_snake_case)]
pub fn WWDebug_Printf_Error(message: &str) {
    dispatch_message(DebugType::Error, message);
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
pub fn WWDebug_Assert_Fail(expr: &str, file: &str, line: i32) {
    let message = format!("{file} ({line}) Assert: {expr}\n");
    let handlers = handlers().lock().expect("debug handler lock poisoned");
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
pub fn WWDebug_Assert_Fail_Print(expr: &str, file: &str, line: i32, string: &str) {
    let message = format!("{file} ({line}) Assert: {expr} {string}\n");
    let handlers = handlers().lock().expect("debug handler lock poisoned");
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
pub fn WWDebug_Check_Trigger(trigger_num: i32) -> bool {
    let handlers = handlers().lock().expect("debug handler lock poisoned");
    if let Some(trigger_handler) = handlers.trigger {
        return trigger_handler(trigger_num);
    }
    false
}

#[allow(non_snake_case)]
pub fn WWDebug_Profile_Start(title: &str) {
    let handlers = handlers().lock().expect("debug handler lock poisoned");
    if let Some(handler) = handlers.profile_start {
        handler(title);
    }
}

#[allow(non_snake_case)]
pub fn WWDebug_Profile_Stop(title: &str) {
    let handlers = handlers().lock().expect("debug handler lock poisoned");
    if let Some(handler) = handlers.profile_stop {
        handler(title);
    }
}

#[allow(non_snake_case)]
pub fn WWDebug_DBWin32_Message_Handler(message: &str) {
    #[cfg(windows)]
    {
        use std::ffi::CString;
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Diagnostics::Debug::OutputDebugStringA;
        use windows_sys::Win32::System::Memory::{
            CreateFileMappingA, MapViewOfFile, FILE_MAP_WRITE, PAGE_READWRITE,
        };
        use windows_sys::Win32::System::Threading::{
            OpenEventA, SetEvent, WaitForSingleObject, EVENT_MODIFY_STATE, INFINITE,
        };

        let ready_event =
            unsafe { OpenEventA(EVENT_MODIFY_STATE, 0, b"DBWIN_BUFFER_READY\0".as_ptr()) };
        if ready_event == 0 {
            unsafe { OutputDebugStringA(b"DBWIN_BUFFER_READY missing\0".as_ptr()) };
            return;
        }
        let data_event =
            unsafe { OpenEventA(EVENT_MODIFY_STATE, 0, b"DBWIN_DATA_READY\0".as_ptr()) };
        if data_event == 0 {
            unsafe { CloseHandle(ready_event) };
            return;
        }

        let mapping = unsafe {
            CreateFileMappingA(
                -1isize as _,
                std::ptr::null_mut(),
                PAGE_READWRITE,
                0,
                4096,
                b"DBWIN_BUFFER\0".as_ptr(),
            )
        };
        if mapping == 0 {
            unsafe {
                CloseHandle(ready_event);
                CloseHandle(data_event);
            }
            return;
        }

        let view = unsafe { MapViewOfFile(mapping, FILE_MAP_WRITE, 0, 0, 512) };
        if view.is_null() {
            unsafe {
                CloseHandle(mapping);
                CloseHandle(ready_event);
                CloseHandle(data_event);
            }
            return;
        }

        let _ = unsafe { WaitForSingleObject(ready_event, INFINITE) };
        let payload = CString::new(message).unwrap_or_else(|_| CString::new("").unwrap());
        unsafe {
            let buffer = view as *mut u8;
            *(buffer as *mut u32) = 0;
            std::ptr::copy_nonoverlapping(
                payload.as_ptr() as *const u8,
                buffer.add(std::mem::size_of::<u32>()),
                payload.as_bytes_with_nul().len(),
            );
            SetEvent(data_event);
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
