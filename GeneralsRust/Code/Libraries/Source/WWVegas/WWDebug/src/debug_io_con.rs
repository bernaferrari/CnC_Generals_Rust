use crate::debug_debug::Debug;
use crate::debug_io::{DebugIOInterface, StringType};
use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;

struct InputState {
    queue: VecDeque<u8>,
}

pub struct DebugIOCon {
    input: Arc<Mutex<InputState>>,
}

impl DebugIOCon {
    pub fn create() -> Box<dyn DebugIOInterface> {
        Box::new(Self::new())
    }

    fn new() -> Self {
        let input = Arc::new(Mutex::new(InputState {
            queue: VecDeque::new(),
        }));
        let thread_input = input.clone();
        thread::spawn(move || {
            let mut stdin = io::stdin();
            let mut buffer = [0u8; 1];
            loop {
                match stdin.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(_) => {
                        let mut guard = thread_input.lock().unwrap();
                        guard.queue.push_back(buffer[0]);
                    }
                    Err(_) => break,
                }
            }
        });
        Self { input }
    }
}

impl DebugIOInterface for DebugIOCon {
    fn read(&mut self, buf: &mut [u8]) -> usize {
        let mut guard = self.input.lock().unwrap();
        let mut count = 0;
        while count < buf.len() {
            match guard.queue.pop_front() {
                Some(byte) => {
                    buf[count] = byte;
                    count += 1;
                    if byte == b'\n' {
                        break;
                    }
                }
                None => break,
            }
        }
        count
    }

    fn write(&mut self, kind: StringType, _src: Option<&str>, message: Option<&str>) {
        if kind == StringType::StructuredCmdReply {
            return;
        }
        if let Some(message) = message {
            let mut stdout = io::stdout();
            let _ = stdout.write_all(message.as_bytes());
            let _ = stdout.flush();
        }
    }

    fn emergency_flush(&mut self) {}

    fn execute(&mut self, dbg: &mut Debug, cmd: &str, _structured: bool, argv: &[&str]) {
        if cmd == "help" {
            dbg.write_plain(
                "con I/O help:\n  add [ <width> [ <height> ] ]\n    console is always active on Rust builds\n",
            );
            return;
        }

        if cmd == "add" {
            let _ = argv;
        }
    }

    fn delete(self: Box<Self>) {}
}
