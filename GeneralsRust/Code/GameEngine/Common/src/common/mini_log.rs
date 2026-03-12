// mini_log.rs - Minimal logging system placeholder

use std::fs::File;
use std::io::Write;
use std::sync::Mutex;

/// Mini logging system
pub struct MiniLog {
    log_file: Option<File>,
}

impl Default for MiniLog {
    fn default() -> Self {
        Self::new()
    }
}

impl MiniLog {
    pub fn new() -> Self {
        Self { log_file: None }
    }

    pub fn init(&mut self, filename: &str) -> Result<(), std::io::Error> {
        self.log_file = Some(File::create(filename)?);
        Ok(())
    }

    pub fn log(&mut self, message: &str) {
        if let Some(ref mut file) = self.log_file {
            let _ = writeln!(file, "{}", message);
            let _ = file.flush();
        }
        println!("{}", message);
    }
}

/// Global logger instance
static THE_MINI_LOG: Mutex<MiniLog> = Mutex::new(MiniLog { log_file: None });

/// Log a message globally
pub fn log_message(message: &str) {
    THE_MINI_LOG.lock().unwrap().log(message);
}
