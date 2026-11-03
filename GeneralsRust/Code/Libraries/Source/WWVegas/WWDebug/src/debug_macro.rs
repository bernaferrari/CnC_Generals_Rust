
pub fn dlog(session: &crate::WwDebugSession, group: &str, message: &str) {
    let mut dbg = session.debug.lock().unwrap();
    if dbg.log_begin(group) {
        dbg.write_plain(message);
        dbg.log_done();
    }
}

pub fn dcheck(session: &crate::WwDebugSession, expr: bool, file: &str, line: i32, message: Option<&str>) -> bool {
    if expr {
        return true;
    }
    let mut dbg = session.debug.lock().unwrap();
    if dbg.check_begin(file, line, "check") {
        if let Some(message) = message {
            dbg.write_plain(message);
        }
        dbg.check_done();
    }
    false
}

pub fn dassert(session: &crate::WwDebugSession, expr: bool, file: &str, line: i32, message: Option<&str>) -> bool {
    if expr {
        return true;
    }
    let mut dbg = session.debug.lock().unwrap();
    if dbg.assert_begin(file, line, "assert") {
        if let Some(message) = message {
            dbg.write_plain(message);
        }
        dbg.assert_done();
    }
    false
}

pub fn dcrash(session: &crate::WwDebugSession, file: Option<&str>, line: Option<i32>, message: &str, die: bool) -> bool {
    let mut dbg = session.debug.lock().unwrap();
    dbg.crash_begin(file, line);
    dbg.write_plain(message);
    dbg.crash_done(die)
}

#[macro_export]
macro_rules! dassert {
    ($session:expr, $expr:expr) => {{
        $crate::debug_macro::dassert(&$session, $expr, file!(), line!() as i32, None)
    }};
    ($session:expr, $expr:expr, $($arg:tt)+) => {{
        $crate::debug_macro::dassert(&$session, $expr, file!(), line!() as i32, Some(&format!($($arg)+)))
    }};
}

#[macro_export]
macro_rules! dcheck {
    ($session:expr, $expr:expr) => {{
        $crate::debug_macro::dcheck(&$session, $expr, file!(), line!() as i32, None)
    }};
    ($session:expr, $expr:expr, $($arg:tt)+) => {{
        $crate::debug_macro::dcheck(&$session, $expr, file!(), line!() as i32, Some(&format!($($arg)+)))
    }};
}

#[macro_export]
macro_rules! dlog {
    ($session:expr, $group:expr, $($arg:tt)+) => {{
        $crate::debug_macro::dlog(&$session, $group, &format!($($arg)+))
    }};
}

#[macro_export]
macro_rules! dcrash {
    ($session:expr, $($arg:tt)+) => {{
        $crate::debug_macro::dcrash(&$session, Some(file!()), Some(line!() as i32), &format!($($arg)+), true)
    }};
}
