////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

// FILE: stack_dump.rs /////////////////////////////////////////////////////////
// Stack dump and debugging utilities
///////////////////////////////////////////////////////////////////////////////

use once_cell::sync::OnceCell;
use parking_lot::RwLock;
use std::backtrace::{Backtrace, BacktraceStatus};
use std::fmt;
use std::io::{self, Write};

/// Stack frame information
#[derive(Debug, Clone)]
pub struct StackFrame {
    pub function_name: Option<String>,
    pub file_name: Option<String>,
    pub line_number: Option<u32>,
    pub address: usize,
}

/// Stack dump utility
pub struct StackDump {
    frames: Vec<StackFrame>,
    capture_time: std::time::SystemTime,
}

impl StackDump {
    /// Capture current stack trace using `std::backtrace`
    pub fn capture() -> Self {
        let backtrace = Backtrace::capture();
        let mut frames = Vec::new();

        match backtrace.status() {
            BacktraceStatus::Captured => {
                // std::backtrace output lines:
                //   0: function_name
                //       at /path/to/file.rs:123
                //   or:
                //   0: 0xaddress
                let bt_str = backtrace.to_string();
                for line in bt_str.lines() {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    if let Some(colon_pos) = trimmed.find(':') {
                        let idx_part = &trimmed[..colon_pos];
                        if idx_part.trim().parse::<usize>().is_ok() {
                            let rest = trimmed[colon_pos + 1..].trim();
                            if rest.starts_with("0x") {
                                if let Ok(addr) = usize::from_str_radix(rest.trim_start_matches("0x"), 16) {
                                    frames.push(StackFrame {
                                        function_name: None,
                                        file_name: None,
                                        line_number: None,
                                        address: addr,
                                    });
                                }
                            } else {
                                frames.push(StackFrame {
                                    function_name: Some(rest.to_string()),
                                    file_name: None,
                                    line_number: None,
                                    address: 0,
                                });
                            }
                        } else if trimmed.starts_with("at ") || trimmed.starts_with('/') {
                            let file_line = if trimmed.starts_with("at ") {
                                &trimmed[3..]
                            } else {
                                trimmed
                            };
                            if let Some(last_frame) = frames.last_mut() {
                                if let Some(colon_pos) = file_line.rfind(':') {
                                    let file_part = &file_line[..colon_pos];
                                    let line_part = &file_line[colon_pos + 1..];
                                    if let Ok(line_num) = line_part.trim().parse::<u32>() {
                                        last_frame.file_name = Some(file_part.trim().to_string());
                                        last_frame.line_number = Some(line_num);
                                    } else {
                                        last_frame.file_name = Some(file_line.trim().to_string());
                                    }
                                } else {
                                    last_frame.file_name = Some(file_line.trim().to_string());
                                }
                            }
                        }
                    }
                }
            }
            BacktraceStatus::Disabled => {
                frames.push(StackFrame {
                    function_name: Some("<backtrace disabled - set RUST_BACKTRACE=1>".to_string()),
                    file_name: None,
                    line_number: None,
                    address: 0,
                });
            }
            _ => {
                frames.push(StackFrame {
                    function_name: Some("<backtrace unavailable>".to_string()),
                    file_name: None,
                    line_number: None,
                    address: 0,
                });
            }
        }

        Self {
            frames,
            capture_time: std::time::SystemTime::now(),
        }
    }

    /// Get the captured frames
    pub fn frames(&self) -> &[StackFrame] {
        &self.frames
    }

    /// Get capture time
    pub fn capture_time(&self) -> std::time::SystemTime {
        self.capture_time
    }

    /// Write stack dump to a writer
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        writeln!(writer, "Stack Dump (captured at {:?}):", self.capture_time)?;
        writeln!(writer, "----------------------------------------")?;

        for (i, frame) in self.frames.iter().enumerate() {
            writeln!(writer, "Frame {}: {}", i, frame)?;
        }

        Ok(())
    }

    /// Convert to string representation
    pub fn to_string(&self) -> String {
        let mut buffer = Vec::new();
        self.write_to(&mut buffer).unwrap_or(());
        String::from_utf8_lossy(&buffer).into_owned()
    }
}

impl fmt::Display for StackFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:08x}", self.address)?;

        if let Some(ref func_name) = self.function_name {
            write!(f, " in {}", func_name)?;
        }

        if let (Some(ref file), Some(line)) = (&self.file_name, self.line_number) {
            write!(f, " at {}:{}", file, line)?;
        }

        Ok(())
    }
}

/// Global stack dump handler
static STACK_DUMP_HANDLER: OnceCell<RwLock<Option<fn(&StackDump)>>> = OnceCell::new();

/// Set global stack dump handler
pub fn set_stack_dump_handler(handler: fn(&StackDump)) {
    STACK_DUMP_HANDLER
        .get_or_init(|| RwLock::new(None))
        .write()
        .replace(handler);
}

/// Trigger stack dump with optional message
pub fn dump_stack(message: Option<&str>) {
    let stack_dump = StackDump::capture();

    if let Some(msg) = message {
        eprintln!("Stack dump triggered: {}", msg);
    }

    eprintln!("{}", stack_dump.to_string());

    if let Some(handler_lock) = STACK_DUMP_HANDLER.get() {
        if let Some(handler) = *handler_lock.read() {
            handler(&stack_dump);
        }
    }
}

/// Dump stack on panic (install as panic hook)
pub fn install_panic_handler() {
    std::panic::set_hook(Box::new(|panic_info| {
        eprintln!("Panic occurred: {}", panic_info);
        dump_stack(Some("Panic triggered"));
    }));
}

/// Stack dump builder for custom scenarios
pub struct StackDumpBuilder {
    include_addresses: bool,
    include_symbols: bool,
    max_frames: Option<usize>,
}

impl StackDumpBuilder {
    pub fn new() -> Self {
        Self {
            include_addresses: true,
            include_symbols: true,
            max_frames: None,
        }
    }

    pub fn include_addresses(mut self, include: bool) -> Self {
        self.include_addresses = include;
        self
    }

    pub fn include_symbols(mut self, include: bool) -> Self {
        self.include_symbols = include;
        self
    }

    pub fn max_frames(mut self, max: usize) -> Self {
        self.max_frames = Some(max);
        self
    }

    pub fn capture(self) -> StackDump {
        // For now, just return a basic stack dump
        // In a full implementation, this would use the builder settings
        StackDump::capture()
    }
}

impl Default for StackDumpBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stack_dump_capture() {
        let stack_dump = StackDump::capture();
        assert!(!stack_dump.frames().is_empty());
    }

    #[test]
    fn test_stack_frame_display() {
        let frame = StackFrame {
            function_name: Some("test_function".to_string()),
            file_name: Some("test.rs".to_string()),
            line_number: Some(42),
            address: 0x1234,
        };

        let display = format!("{}", frame);
        assert!(display.contains("0x00001234"));
        assert!(display.contains("test_function"));
        assert!(display.contains("test.rs:42"));
    }

    #[test]
    fn test_stack_dump_to_string() {
        let stack_dump = StackDump::capture();
        let string_repr = stack_dump.to_string();
        assert!(string_repr.contains("Stack Dump"));
    }

    #[test]
    fn test_stack_dump_builder() {
        let builder = StackDumpBuilder::new()
            .include_addresses(false)
            .max_frames(10);

        let stack_dump = builder.capture();
        assert!(!stack_dump.frames().is_empty());
    }
}
