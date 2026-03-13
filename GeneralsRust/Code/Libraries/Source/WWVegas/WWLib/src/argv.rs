//! Argument vector handling mirroring WWLib `argv.h` / `argv.cpp`.
//!
//! Used to parse command line that is passed into WinMain.
//! It also has the ability to load a file with values to append to the command line.
//! Normally in WinMain() there would be a call `ArgvClass::Init(lpCmdLine, fileprefix)`.
//! Once this is done, user can create an ArgvClass object (say argv) then argv.find() can be called.
//! If there is an argument `<fileprefix><fname>` (for example @file.arg) then the fname is loaded up,
//! parsed, and put into the command line.
//!
//! The format of the parameter file is as follows:
//! 1. A semicolon (;) or hash (#) at the start of the line is a comment and will be ignored.
//! 2. Each line is a separate parameter. This enables white space to be embedded.
//! In typical Argv implementation, the first argument is the name of the application.
//! This is not the case with this.

use std::sync::{Mutex, OnceLock};

/// Maximum number of command-line arguments.
const MAX_ARGC: usize = 256;

/// Global argument storage matching C++ static members.
struct GlobalArgs {
    argc: usize,
    argv: Vec<String>,
}

impl GlobalArgs {
    fn new() -> Self {
        GlobalArgs {
            argc: 0,
            argv: Vec::with_capacity(MAX_ARGC),
        }
    }
}

/// Get the global argument storage instance.
fn global_args() -> &'static Mutex<GlobalArgs> {
    static INSTANCE: OnceLock<Mutex<GlobalArgs>> = OnceLock::new();
    INSTANCE.get_or_init(|| Mutex::new(GlobalArgs::new()))
}

/// Flags for argument matching behavior.
#[derive(Clone, Copy, Debug)]
pub struct ArgvFlags {
    /// Perform case-sensitive search.
    pub case_sensitive: bool,
    /// Require exact string length match.
    pub exact_size: bool,
}

impl Default for ArgvFlags {
    fn default() -> Self {
        ArgvFlags {
            case_sensitive: false,
            exact_size: false,
        }
    }
}

/// Command-line argument parser.
///
/// This class provides functionality to parse, search, and manipulate
/// command-line arguments, including loading arguments from files.
pub struct ArgvClass {
    /// Current position for iteration.
    current_pos: isize,
    /// Last argument being searched for.
    last_arg: Option<String>,
    /// Behavior flags.
    flags: ArgvFlags,
}

impl ArgvClass {
    /// Initialize the command line parsing system.
    ///
    /// Matches C++ `static int Init(char *lpCmdLine, char *fileprefix = "@")`.
    ///
    /// Should be called before any objects are created.
    /// This can be called multiple times.
    ///
    /// # Arguments
    ///
    /// * `cmd_line` - A string of white-space-separated strings. Quotes force spaces to be ignored.
    /// * `fileprefix` - A prefix on an argument telling system to load postfix file name
    ///                  as command line params. Default is "@".
    ///
    /// # Returns
    ///
    /// Number of parameters read in.
    pub fn init(cmd_line: &str, fileprefix: Option<&str>) -> usize {
        let fp = fileprefix.unwrap_or("@");
        let global = global_args();
        let mut args = global.lock().unwrap();
        let orig_argc = args.argc;

        let mut ptr = cmd_line;
        let fp_cmp_len = fp.len();

        while !ptr.is_empty() {
            // Skip leading whitespace
            while ptr.starts_with(|c: char| c.is_whitespace()) {
                ptr = &ptr[1..];
                if ptr.is_empty() {
                    break;
                }
            }
            if ptr.is_empty() {
                break;
            }

            let (token, remaining) = if ptr.starts_with('"') {
                // Quoted string - find closing quote
                let rest = &ptr[1..];
                if let Some(end) = rest.find('"') {
                    (&rest[..end], &rest[end + 1..])
                } else {
                    (rest, "")
                }
            } else {
                // Unquoted - find next whitespace
                if let Some(end) = ptr.find(|c: char| c.is_whitespace()) {
                    (&ptr[..end], &ptr[end..])
                } else {
                    (ptr, "")
                }
            };

            // Check if this is a file prefix argument
            let mut was_file = false;
            if fp_cmp_len > 0 && token.starts_with(fp) {
                let filename = &token[fp_cmp_len..];
                if !filename.is_empty() {
                    was_file = Self::load_file_inner(&mut args, filename);
                }
            }

            if !was_file && !token.is_empty() && args.argc < MAX_ARGC {
                args.argv.push(token.to_string());
                args.argc += 1;
            }

            ptr = remaining;
            if ptr.is_empty() {
                break;
            }
        }

        args.argc - orig_argc
    }

    /// Load arguments from a file.
    ///
    /// Matches C++ `static bool Load_File(const char *fname)`.
    fn load_file_inner(args: &mut GlobalArgs, fname: &str) -> bool {
        let content = match std::fs::read_to_string(fname) {
            Ok(c) => c,
            Err(_) => return false,
        };

        for line in content.lines() {
            if args.argc >= MAX_ARGC {
                break;
            }

            // Skip comments (lines starting with # or ;)
            if line.starts_with('#') || line.starts_with(';') {
                continue;
            }

            // Trim trailing whitespace
            let trimmed = line.trim_end();

            // Skip empty lines
            if trimmed.is_empty() {
                continue;
            }

            args.argv.push(trimmed.to_string());
            args.argc += 1;
        }

        true
    }

    /// Load arguments from a file (public interface).
    ///
    /// Matches C++ `static bool Load_File(const char *fname)`.
    pub fn load_file(fname: &str) -> bool {
        let global = global_args();
        let mut args = global.lock().unwrap();
        Self::load_file_inner(&mut args, fname)
    }

    /// Release data allocated.
    ///
    /// Matches C++ `static void Free()`.
    pub fn free() {
        let global = global_args();
        let mut args = global.lock().unwrap();
        args.argv.clear();
        args.argc = 0;
    }

    /// Create an instance to parse argv with.
    ///
    /// Matches C++ `ArgvClass(bool case_sensitive = false, bool exact_size = false)`.
    ///
    /// # Arguments
    ///
    /// * `case_sensitive` - Do you want to perform a case sensitive search?
    /// * `exact_size` - Do you want string of same length?
    pub fn new(case_sensitive: bool, exact_size: bool) -> Self {
        ArgvClass {
            current_pos: -1,
            last_arg: None,
            flags: ArgvFlags {
                case_sensitive,
                exact_size,
            },
        }
    }

    /// Search for a string given the flags.
    ///
    /// Matches C++ `const char *Find(const char *arg)`.
    ///
    /// Returns the found string, or None if not found.
    pub fn find(&mut self, arg: &str) -> Option<String> {
        self.current_pos = -1;
        self.find_again(Some(arg))
    }

    /// Continue searching for the same or new string.
    ///
    /// Matches C++ `const char *Find_Again(const char *arg = 0L)`.
    ///
    /// If None is passed, the last search string will be used.
    pub fn find_again(&mut self, arg: Option<&str>) -> Option<String> {
        if let Some(a) = arg {
            self.last_arg = Some(a.to_string());
        }

        let search_arg = match &self.last_arg {
            Some(a) => a.clone(),
            None => return None,
        };

        let global = global_args();
        let args = global.lock().unwrap();

        self.current_pos += 1;
        let mut pos = self.current_pos as usize;

        if pos < args.argc {
            if self.flags.case_sensitive {
                if self.flags.exact_size {
                    // Case Sensitive, Exact Size
                    while pos < args.argc {
                        if search_arg == args.argv[pos] {
                            self.current_pos = pos as isize;
                            return Some(args.argv[pos].clone());
                        }
                        pos += 1;
                    }
                } else {
                    // Case Sensitive, Match first chars
                    while pos < args.argc {
                        if args.argv[pos].starts_with(&search_arg) {
                            self.current_pos = pos as isize;
                            return Some(args.argv[pos].clone());
                        }
                        pos += 1;
                    }
                }
            } else {
                let search_lower = search_arg.to_lowercase();
                if self.flags.exact_size {
                    // Case Insensitive, Exact Size
                    while pos < args.argc {
                        if args.argv[pos].len() == search_arg.len()
                            && args.argv[pos].to_lowercase() == search_lower
                        {
                            self.current_pos = pos as isize;
                            return Some(args.argv[pos].clone());
                        }
                        pos += 1;
                    }
                } else {
                    // Case Insensitive, Match first chars
                    while pos < args.argc {
                        let arg_lower = args.argv[pos].to_lowercase();
                        if arg_lower.starts_with(&search_lower) {
                            self.current_pos = pos as isize;
                            return Some(args.argv[pos].clone());
                        }
                        pos += 1;
                    }
                }
            }
        }

        None
    }

    /// Find value of argument given prefix.
    ///
    /// Matches C++ `const char *Find_Value(const char *arg)`.
    ///
    /// Returns the value after the prefix, or None.
    pub fn find_value(&mut self, arg: &str) -> Option<String> {
        if arg.is_empty() {
            return None;
        }
        if self.find(arg).is_some() {
            self.get_cur_value(arg.len())
        } else {
            None
        }
    }

    /// Get value of current argument.
    ///
    /// Matches C++ `const char *Get_Cur_Value(unsigned prefixlen, bool * val_in_next = 0)`.
    ///
    /// Returns the value and whether it was found in the next argument.
    pub fn get_cur_value_with_flag(&self, prefixlen: usize) -> (Option<String>, bool) {
        if self.current_pos < 0 {
            return (None, false);
        }

        let global = global_args();
        let args = global.lock().unwrap();
        let pos = self.current_pos as usize;

        if pos >= args.argc {
            return (None, false);
        }

        let current = &args.argv[pos];

        if current.len() < prefixlen {
            return (None, false);
        }

        // Look for non-whitespace after prefix
        let after_prefix = &current[prefixlen..];
        let trimmed = after_prefix.trim_start();
        if !trimmed.is_empty() {
            return (Some(trimmed.to_string()), false);
        }

        // Check next argument
        if pos + 1 < args.argc {
            let next = &args.argv[pos + 1];
            let next_trimmed = next.trim_start();
            if !next_trimmed.is_empty() {
                return (Some(next_trimmed.to_string()), true);
            }
        }

        (None, false)
    }

    /// Get value of current argument.
    ///
    /// Matches C++ `const char *Get_Cur_Value(unsigned prefixlen, bool * val_in_next = 0)`.
    pub fn get_cur_value(&self, prefixlen: usize) -> Option<String> {
        self.get_cur_value_with_flag(prefixlen).0
    }

    /// Update an existing attrib to use this new value.
    ///
    /// Matches C++ `void Update_Value(const char *attrib, const char *value)`.
    pub fn update_value(&mut self, attrib: &str, value: &str) {
        if self.find_value(attrib).is_some() {
            let global = global_args();
            let mut args = global.lock().unwrap();
            let pos = self.current_pos as usize;

            if pos + 1 < args.argc && !args.argv[pos + 1].starts_with('-') {
                // Update old value
                args.argv[pos + 1] = value.to_string();
            } else {
                // Add new value - insert after current position
                args.argv.insert(pos + 1, value.to_string());
                args.argc += 1;
                if args.argc > MAX_ARGC {
                    args.argc = MAX_ARGC;
                    args.argv.truncate(MAX_ARGC);
                }
            }
        } else {
            // Just add the new stuff
            self.add_value(attrib, Some(value));
        }
    }

    /// Add a new attrib value pair (or just an option).
    ///
    /// Matches C++ `void Add_Value(const char *attrib, const char *value=NULL)`.
    pub fn add_value(&mut self, attrib: &str, value: Option<&str>) {
        if attrib.is_empty() {
            return;
        }

        let global = global_args();
        let mut args = global.lock().unwrap();

        if args.argc < MAX_ARGC {
            args.argv.push(attrib.to_string());
            args.argc += 1;
        }

        if let Some(v) = value {
            if args.argc < MAX_ARGC {
                args.argv.push(v.to_string());
                args.argc += 1;
            }
        }
    }

    /// Remove an option (and its value).
    ///
    /// Matches C++ `bool Remove_Value(const char *attrib)`.
    ///
    /// Note: Contains the same potential bug as C++ - if argv[0] = "-i test" and "*.txt"
    /// as values, calling remove_value("-i") will remove *.txt as well.
    pub fn remove_value(&mut self, attrib: &str) -> bool {
        if self.find_value(attrib).is_some() {
            let global = global_args();
            let mut args = global.lock().unwrap();
            let pos = self.current_pos as usize;

            let mut remove_count = 1;
            if pos + 1 < args.argc && !args.argv[pos + 1].starts_with('-') {
                remove_count = 2;
            }

            for _ in 0..remove_count {
                if pos < args.argc {
                    args.argv.remove(pos);
                    args.argc -= 1;
                }
            }

            true
        } else {
            false
        }
    }

    /// Get the first parameter.
    ///
    /// Matches C++ `const char *First()`.
    pub fn first(&mut self) -> Option<String> {
        self.current_pos = 0;
        self.cur()
    }

    /// Get the next parameter.
    ///
    /// Matches C++ `const char *Next()`.
    /// Works after a find() call also.
    pub fn next(&mut self) -> Option<String> {
        self.current_pos += 1;
        self.cur()
    }

    /// Reset iteration to before the first element.
    ///
    /// Matches C++ `void Reset()`.
    /// Can be called so next() will return first().
    pub fn reset(&mut self) {
        self.current_pos = -1;
    }

    /// Get the current parameter.
    ///
    /// Matches C++ `const char *Cur()`.
    pub fn cur(&self) -> Option<String> {
        let global = global_args();
        let args = global.lock().unwrap();
        let pos = self.current_pos as usize;

        if pos < args.argc {
            Some(args.argv[pos].clone())
        } else {
            None
        }
    }

    /// Allow user to change case sensitivity.
    ///
    /// Matches C++ `void Case_Sensitive(bool on)`.
    pub fn set_case_sensitive(&mut self, on: bool) {
        self.flags.case_sensitive = on;
    }

    /// Check if case sensitive mode is on.
    ///
    /// Matches C++ `bool Is_Case_Sensitive()`.
    pub fn is_case_sensitive(&self) -> bool {
        self.flags.case_sensitive
    }

    /// Allow user to change exact size matching.
    ///
    /// Matches C++ `void Exact_Size(bool on)`.
    pub fn set_exact_size(&mut self, on: bool) {
        self.flags.exact_size = on;
    }

    /// Check if exact size mode is on.
    ///
    /// Matches C++ `bool Is_Exact_Size()`.
    pub fn is_exact_size(&self) -> bool {
        self.flags.exact_size
    }

    /// Get the current position in the argument list.
    pub fn current_pos(&self) -> isize {
        self.current_pos
    }
}

impl Default for ArgvClass {
    fn default() -> Self {
        ArgvClass::new(false, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_args() {
        ArgvClass::free();
    }

    #[test]
    fn test_init_simple() {
        setup_args();
        let count = ArgvClass::init("-flag value -other", None);
        assert_eq!(count, 3);
    }

    #[test]
    fn test_init_quoted() {
        setup_args();
        let count = ArgvClass::init("\"quoted arg\" -flag", None);
        assert_eq!(count, 2);
    }

    #[test]
    fn test_find() {
        setup_args();
        ArgvClass::init("-flag value -other", None);

        let mut argv = ArgvClass::new(false, false);
        let result = argv.find("-flag");
        assert_eq!(result, Some("-flag".to_string()));
    }

    #[test]
    fn test_find_value() {
        setup_args();
        ArgvClass::init("-flag value -other", None);

        let mut argv = ArgvClass::new(false, false);
        let value = argv.find_value("-flag");
        assert_eq!(value, Some("value".to_string()));
    }

    #[test]
    fn test_case_insensitive() {
        setup_args();
        ArgvClass::init("-FLAG value", None);

        let mut argv = ArgvClass::new(false, false);
        let result = argv.find("-flag");
        assert_eq!(result, Some("-FLAG".to_string()));
    }

    #[test]
    fn test_case_sensitive() {
        setup_args();
        ArgvClass::init("-FLAG value", None);

        let mut argv = ArgvClass::new(true, false);
        let result = argv.find("-flag");
        assert_eq!(result, None);

        let result = argv.find("-FLAG");
        assert_eq!(result, Some("-FLAG".to_string()));
    }

    #[test]
    fn test_exact_size() {
        setup_args();
        ArgvClass::init("-flag value", None);

        let mut argv = ArgvClass::new(false, true);
        let result = argv.find("-fla"); // Prefix only, shouldn't match with exact_size
        assert_eq!(result, None);

        let result = argv.find("-flag");
        assert_eq!(result, Some("-flag".to_string()));
    }

    #[test]
    fn test_first_next() {
        setup_args();
        ArgvClass::init("first second third", None);

        let mut argv = ArgvClass::new(false, false);
        assert_eq!(argv.first(), Some("first".to_string()));
        assert_eq!(argv.next(), Some("second".to_string()));
        assert_eq!(argv.next(), Some("third".to_string()));
        assert_eq!(argv.next(), None);
    }

    #[test]
    fn test_add_value() {
        setup_args();
        ArgvClass::init("initial", None);

        let mut argv = ArgvClass::new(false, false);
        argv.add_value("-new", Some("val"));

        let value = argv.find_value("-new");
        assert_eq!(value, Some("val".to_string()));
    }

    #[test]
    fn test_free() {
        setup_args();
        ArgvClass::init("something", None);
        ArgvClass::free();

        let mut argv = ArgvClass::new(false, false);
        assert_eq!(argv.first(), None);
    }

    #[test]
    fn test_find_again() {
        setup_args();
        ArgvClass::init("-test one -test two", None);

        let mut argv = ArgvClass::new(false, false);
        let first = argv.find("-test");
        assert_eq!(first, Some("-test".to_string()));

        let second = argv.find_again(None);
        assert_eq!(second, Some("-test".to_string()));
    }
}
