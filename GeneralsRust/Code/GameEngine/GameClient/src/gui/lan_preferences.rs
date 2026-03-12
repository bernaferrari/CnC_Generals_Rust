//! LAN preference storage (Network.ini).

use std::collections::HashMap;
use std::fs::{create_dir_all, File};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

const NUM_REMOTE_IPS_KEY: &str = "NumRemoteIPs";
const USER_NAME_KEY: &str = "UserName";

#[derive(Debug, Default)]
pub struct LanPreferences {
    data: HashMap<String, String>,
}

impl LanPreferences {
    pub fn new() -> Self {
        let mut prefs = Self {
            data: HashMap::new(),
        };
        prefs.read_data();
        prefs
    }

    pub fn write(&self) {
        let path = preferences_file();
        if let Some(parent) = path.parent() {
            let _ = create_dir_all(parent);
        }
        if let Ok(mut file) = File::create(&path) {
            for (key, value) in &self.data {
                let _ = writeln!(file, "{}={}", key, value);
            }
        }
    }

    fn read_data(&mut self) {
        let path = preferences_file();
        let file = match File::open(&path) {
            Ok(file) => file,
            Err(_) => return,
        };
        let reader = BufReader::new(file);
        for line in reader.lines().flatten() {
            if let Some((key, value)) = line.split_once('=') {
                self.data
                    .insert(key.trim().to_string(), value.trim().to_string());
            }
        }
    }

    pub fn get_user_name(&self) -> String {
        let stored = self
            .data
            .get(USER_NAME_KEY)
            .map(|value| quoted_printable_decode(value))
            .unwrap_or_default();
        let trimmed = stored.trim();
        if trimmed.is_empty() {
            get_machine_name()
        } else {
            trimmed.to_string()
        }
    }

    pub fn set_user_name(&mut self, value: String) {
        let encoded = quoted_printable_encode(&value);
        self.data.insert(USER_NAME_KEY.to_string(), encoded);
    }

    pub fn get_num_remote_ips(&self) -> i32 {
        self.data
            .get(NUM_REMOTE_IPS_KEY)
            .and_then(|value| value.parse::<i32>().ok())
            .unwrap_or(0)
    }

    pub fn set_num_remote_ips(&mut self, value: i32) {
        self.data
            .insert(NUM_REMOTE_IPS_KEY.to_string(), value.to_string());
    }

    pub fn get_remote_ip_entry(&self, index: i32) -> String {
        let key = format!("RemoteIP{}", index);
        if let Some(entry) = self.data.get(&key) {
            if let Some((ip, desc)) = entry.split_once(':') {
                if !desc.is_empty() {
                    return format!("{}({})", ip, quoted_printable_decode(desc));
                }
                return ip.to_string();
            }
            return entry.clone();
        }
        String::new()
    }

    pub fn set_remote_ip_entry(&mut self, index: i32, value: String) {
        let key = format!("RemoteIP{}", index);
        self.data.insert(key, value);
    }
}

fn preferences_file() -> PathBuf {
    let mut path = if let Some(home) = std::env::var_os("HOME") {
        PathBuf::from(home)
    } else if let Some(appdata) = std::env::var_os("APPDATA") {
        PathBuf::from(appdata)
    } else {
        PathBuf::from(".")
    };
    path.push(".generals");
    path.push("Network.ini");
    path
}

fn get_machine_name() -> String {
    if let Ok(name) = std::env::var("COMPUTERNAME") {
        return name;
    }
    if let Ok(name) = std::env::var("HOSTNAME") {
        return name;
    }
    if let Ok(name) = std::env::var("USER") {
        return name;
    }
    "Player".to_string()
}

fn quoted_printable_encode(input: &str) -> String {
    let mut output = String::new();
    for &byte in input.as_bytes() {
        let ch = byte as char;
        if ch.is_ascii_alphanumeric() || ch == ' ' {
            output.push(ch);
        } else {
            output.push('=');
            output.push_str(&format!("{:02X}", byte));
        }
    }
    output
}

fn quoted_printable_decode(input: &str) -> String {
    let mut output: Vec<u8> = Vec::new();
    let bytes = input.as_bytes();
    let mut idx = 0;
    while idx < bytes.len() {
        let byte = bytes[idx];
        if byte == b'=' && idx + 2 < bytes.len() {
            let hi = bytes[idx + 1] as char;
            let lo = bytes[idx + 2] as char;
            if let Ok(decoded) = u8::from_str_radix(&format!("{}{}", hi, lo), 16) {
                output.push(decoded);
                idx += 3;
                continue;
            }
        }
        output.push(byte);
        idx += 1;
    }
    String::from_utf8_lossy(&output).to_string()
}
