use crate::debug_debug::Debug;
use crate::debug_io::{DebugIOInterface, StringType};
use std::io::{Read, Write};

pub struct DebugIONet {
    #[cfg(windows)]
    pipe: Option<windows_sys::Win32::Foundation::HANDLE>,
    #[cfg(not(windows))]
    stream: Option<std::net::TcpStream>,
}

impl DebugIONet {
    pub fn create() -> Box<dyn DebugIOInterface> {
        Box::new(Self::new())
    }

    fn new() -> Self {
        Self {
            #[cfg(windows)]
            pipe: None,
            #[cfg(not(windows))]
            stream: None,
        }
    }
}

impl DebugIOInterface for DebugIONet {
    fn read(&mut self, buf: &mut [u8]) -> usize {
        #[cfg(windows)]
        {
            use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
            use windows_sys::Win32::Storage::FileSystem::ReadFile;
            use windows_sys::Win32::System::Pipes::{
                SetNamedPipeHandleState, PIPE_NOWAIT, PIPE_READMODE_MESSAGE, PIPE_WAIT,
            };

            let Some(pipe) = self.pipe else {
                return 0;
            };
            if pipe == INVALID_HANDLE_VALUE {
                return 0;
            }

            let mut mode = PIPE_READMODE_MESSAGE | PIPE_NOWAIT;
            unsafe {
                SetNamedPipeHandleState(
                    pipe,
                    &mut mode,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                );
            }
            let mut read = 0u32;
            let ok = unsafe {
                ReadFile(
                    pipe,
                    buf.as_mut_ptr() as _,
                    buf.len() as u32,
                    &mut read,
                    std::ptr::null_mut(),
                )
            };
            mode = PIPE_READMODE_MESSAGE | PIPE_WAIT;
            unsafe {
                SetNamedPipeHandleState(
                    pipe,
                    &mut mode,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                );
            }
            if ok == 0 {
                unsafe {
                    CloseHandle(pipe);
                }
                self.pipe = None;
                return 0;
            }
            read as usize
        }

        #[cfg(not(windows))]
        {
            if let Some(stream) = self.stream.as_mut() {
                stream.set_nonblocking(true).ok();
                match stream.read(buf) {
                    Ok(count) => count,
                    Err(_) => 0,
                }
            } else {
                0
            }
        }
    }

    fn write(&mut self, kind: StringType, src: Option<&str>, message: Option<&str>) {
        let Some(message) = message else {
            return;
        };

        #[cfg(windows)]
        {
            use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
            use windows_sys::Win32::Storage::FileSystem::WriteFile;

            let Some(pipe) = self.pipe else {
                return;
            };
            if pipe == INVALID_HANDLE_VALUE {
                return;
            }
            let mut written = 0u32;
            let kind_byte = kind as u8;
            unsafe {
                WriteFile(
                    pipe,
                    &kind_byte as *const u8 as _,
                    1,
                    &mut written,
                    std::ptr::null_mut(),
                );
            }
            let src_bytes = src.unwrap_or("").as_bytes();
            let src_len = src_bytes.len() as u32;
            unsafe {
                WriteFile(
                    pipe,
                    &src_len as *const u32 as _,
                    4,
                    &mut written,
                    std::ptr::null_mut(),
                );
                if src_len > 0 {
                    WriteFile(
                        pipe,
                        src_bytes.as_ptr() as _,
                        src_len,
                        &mut written,
                        std::ptr::null_mut(),
                    );
                }
            }
            let msg_bytes = message.as_bytes();
            let msg_len = msg_bytes.len() as u32;
            unsafe {
                WriteFile(
                    pipe,
                    &msg_len as *const u32 as _,
                    4,
                    &mut written,
                    std::ptr::null_mut(),
                );
                if msg_len > 0 {
                    WriteFile(
                        pipe,
                        msg_bytes.as_ptr() as _,
                        msg_len,
                        &mut written,
                        std::ptr::null_mut(),
                    );
                }
            }
        }

        #[cfg(not(windows))]
        {
            if let Some(stream) = self.stream.as_mut() {
                let mut payload = Vec::new();
                payload.push(kind as u8);
                let src_bytes = src.unwrap_or("").as_bytes();
                payload.extend_from_slice(&(src_bytes.len() as u32).to_le_bytes());
                payload.extend_from_slice(src_bytes);
                let msg_bytes = message.as_bytes();
                payload.extend_from_slice(&(msg_bytes.len() as u32).to_le_bytes());
                payload.extend_from_slice(msg_bytes);
                let _ = stream.write_all(&payload);
            }
        }
    }

    fn emergency_flush(&mut self) {}

    fn execute(&mut self, dbg: &mut Debug, cmd: &str, _structured: bool, argv: &[&str]) {
        if cmd == "help" {
            dbg.write_plain(
                "net I/O help:\n  add [ <machine> ]\n    connect to a named pipe (Windows) or TCP host (Unix)\n",
            );
            return;
        }

        if cmd == "add" {
            let machine = argv.get(0).copied().unwrap_or(".");
            #[cfg(windows)]
            {
                use std::ffi::CString;
                use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
                use windows_sys::Win32::Storage::FileSystem::{CreateFileA, OPEN_EXISTING};
                use windows_sys::Win32::System::Pipes::{
                    SetNamedPipeHandleState, PIPE_READMODE_MESSAGE,
                };
                use windows_sys::Win32::System::SystemServices::{GENERIC_READ, GENERIC_WRITE};

                let pipe_name = format!(r"\\{machine}\pipe\ea_debug_v1");
                let pipe_name = CString::new(pipe_name).unwrap();
                let handle = unsafe {
                    CreateFileA(
                        pipe_name.as_ptr(),
                        GENERIC_READ | GENERIC_WRITE,
                        0,
                        std::ptr::null_mut(),
                        OPEN_EXISTING,
                        0,
                        0,
                    )
                };
                if handle == INVALID_HANDLE_VALUE {
                    dbg.write_plain("Could not connect to given machine.\n");
                    return;
                }
                let mut mode = PIPE_READMODE_MESSAGE;
                unsafe {
                    SetNamedPipeHandleState(
                        handle,
                        &mut mode,
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                    );
                }
                self.pipe = Some(handle);
                self.write(
                    StringType::Other,
                    None,
                    Some(&format!("Client at {machine}\n")),
                );
            }

            #[cfg(not(windows))]
            {
                let target = if machine.contains(':') {
                    machine.to_string()
                } else {
                    format!("{machine}:2222")
                };
                match std::net::TcpStream::connect(target) {
                    Ok(mut stream) => {
                        let _ = stream.set_nodelay(true);
                        self.stream = Some(stream);
                        self.write(
                            StringType::Other,
                            None,
                            Some(&format!("Client at {machine}\n")),
                        );
                    }
                    Err(_) => {
                        dbg.write_plain("Could not connect to given machine.\n");
                    }
                }
            }
        }
    }

    fn delete(self: Box<Self>) {}
}
