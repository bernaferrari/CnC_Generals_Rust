use crate::debug_debug::Debug;

pub fn dlog(group: &str, message: &str) {
    let mut dbg = Debug::instance().lock().unwrap();
    if dbg.log_begin(group) {
        dbg.write_plain(message);
        dbg.log_done();
    }
}

pub fn dcheck(expr: bool, file: &str, line: i32, message: Option<&str>) -> bool {
    if expr {
        return true;
    }
    let mut dbg = Debug::instance().lock().unwrap();
    if dbg.check_begin(file, line, "check") {
        if let Some(message) = message {
            dbg.write_plain(message);
        }
        dbg.check_done();
    }
    false
}

pub fn dassert(expr: bool, file: &str, line: i32, message: Option<&str>) -> bool {
    if expr {
        return true;
    }
    let mut dbg = Debug::instance().lock().unwrap();
    if dbg.assert_begin(file, line, "assert") {
        if let Some(message) = message {
            dbg.write_plain(message);
        }
        dbg.assert_done();
    }
    false
}

pub fn dcrash(file: Option<&str>, line: Option<i32>, message: &str, die: bool) -> bool {
    let mut dbg = Debug::instance().lock().unwrap();
    dbg.crash_begin(file, line);
    dbg.write_plain(message);
    dbg.crash_done(die)
}

#[macro_export]
macro_rules! dassert {
    ($expr:expr) => {{
        $crate::debug_macro::dassert($expr, file!(), line!() as i32, None)
    }};
    ($expr:expr, $($arg:tt)+) => {{
        $crate::debug_macro::dassert($expr, file!(), line!() as i32, Some(&format!($($arg)+)))
    }};
}

#[macro_export]
macro_rules! dcheck {
    ($expr:expr) => {{
        $crate::debug_macro::dcheck($expr, file!(), line!() as i32, None)
    }};
    ($expr:expr, $($arg:tt)+) => {{
        $crate::debug_macro::dcheck($expr, file!(), line!() as i32, Some(&format!($($arg)+)))
    }};
}

#[macro_export]
macro_rules! dlog {
    ($group:expr, $($arg:tt)+) => {{
        $crate::debug_macro::dlog($group, &format!($($arg)+))
    }};
}

#[macro_export]
macro_rules! dcrash {
    ($($arg:tt)+) => {{
        $crate::debug_macro::dcrash(Some(file!()), Some(line!() as i32), &format!($($arg)+), true)
    }};
}
