//! Profile Command Interface Module
//!
//! Rust implementation of the C++ ProfileCmdInterface functionality.
//! Provides a command-line interface for controlling and configuring profiling.

use crate::result::{ProfileResultInterface, ResultFunctionRegistry};
use crate::{Profile, ProfileError, ProfileResult};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::io::{self, Write};
use std::sync::Mutex;

/// Command execution mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandMode {
    /// Normal interactive mode with full output
    Normal,
    /// Scripted mode with minimal output
    Script,
    /// Query mode for getting values without side effects
    Query,
}

impl Default for CommandMode {
    fn default() -> Self {
        CommandMode::Normal
    }
}

/// Factory function type for creating result interfaces
pub type ResultFactoryFn = fn(&[&str]) -> Option<Box<dyn ProfileResultInterface>>;

/// Result function factory entry
#[derive(Clone)]
struct ResultFactory {
    name: String,
    description: String,
    factory_fn: ResultFactoryFn,
}

/// Global command interface state
struct CommandInterfaceState {
    result_factories: Mutex<HashMap<String, ResultFactory>>,
    registered_result_functions: Mutex<Vec<Box<dyn ProfileResultInterface>>>,
}

static COMMAND_STATE: Lazy<CommandInterfaceState> = Lazy::new(|| {
    let mut state = CommandInterfaceState {
        result_factories: Mutex::new(HashMap::new()),
        registered_result_functions: Mutex::new(Vec::new()),
    };

    // Register default result functions
    ProfileCmdInterface::register_default_result_functions(&mut state);

    state
});

/// Main profile command interface - equivalent to C++ ProfileCmdInterface
pub struct ProfileCmdInterface;

impl ProfileCmdInterface {
    /// Add a result function factory
    ///
    /// # Arguments
    /// * `name` - Name of the result function
    /// * `description` - Description and usage information
    /// * `factory_fn` - Factory function to create the result interface
    pub fn add_result_function(
        name: &str,
        description: &str,
        factory_fn: ResultFactoryFn,
    ) -> ProfileResult<()> {
        let state = &*COMMAND_STATE;
        let mut factories = state.result_factories.lock().unwrap();

        // Don't add duplicates
        if factories.contains_key(name) {
            return Ok(());
        }

        factories.insert(
            name.to_string(),
            ResultFactory {
                name: name.to_string(),
                description: description.to_string(),
                factory_fn,
            },
        );

        Ok(())
    }

    /// Execute result functions (typically called on program exit)
    pub fn run_result_functions() {
        let state = &*COMMAND_STATE;
        let mut result_functions = state.registered_result_functions.lock().unwrap();

        // If no result functions registered, add default CSV output
        if result_functions.is_empty() {
            if let Some(csv_writer) = ResultFunctionRegistry::create_function("file_csv", &[]) {
                result_functions.push(csv_writer);
            }
        }

        // Execute all result functions
        for result_fn in result_functions.drain(..) {
            result_fn.write_results();
        }
    }

    /// Execute a profile command
    ///
    /// # Arguments
    /// * `writer` - Output writer (e.g., stdout, stderr, etc.)
    /// * `cmd` - Command name
    /// * `cmd_mode` - Command execution mode
    /// * `args` - Command arguments
    ///
    /// # Returns
    /// `true` if command was recognized and handled, `false` otherwise
    pub fn execute<W: Write>(
        writer: &mut W,
        cmd: &str,
        cmd_mode: CommandMode,
        args: &[&str],
    ) -> ProfileResult<bool> {
        let normal_mode = cmd_mode == CommandMode::Normal;

        match cmd {
            "help" => Self::handle_help_command(writer, normal_mode, args),
            "result" => Self::handle_result_command(writer, normal_mode, args),
            "caller" => Self::handle_caller_command(writer, normal_mode, args),
            "clear" => Self::handle_clear_command(writer, normal_mode, args),
            "add" => Self::handle_add_command(writer, normal_mode, args),
            "view" => Self::handle_view_command(writer, normal_mode, args),
            _ => Ok(false), // Unknown command
        }
    }

    /// Handle the "help" command
    fn handle_help_command<W: Write>(
        writer: &mut W,
        normal_mode: bool,
        args: &[&str],
    ) -> ProfileResult<bool> {
        if !normal_mode {
            return Ok(true);
        }

        if args.is_empty() {
            writeln!(writer, "profile group help:")?;
            writeln!(writer, "  result, caller, clear, add, view")?;
        } else {
            match args[0] {
                "result" => {
                    writeln!(writer, "result")?;
                    writeln!(writer)?;
                    writeln!(
                        writer,
                        "Shows the list of available result functions and their"
                    )?;
                    writeln!(writer, "optional parameters.")?;
                    writeln!(writer)?;
                    writeln!(writer, "result <res_func_name> [ <arg1> .. <argN> ]")?;
                    writeln!(writer)?;
                    writeln!(
                        writer,
                        "Adds the given result function to be executed on program"
                    )?;
                    writeln!(writer, "exit.")?;
                }
                "caller" => {
                    writeln!(writer, "caller [ (+|-) ]")?;
                    writeln!(writer)?;
                    writeln!(
                        writer,
                        "Enables/disables recording of caller information while"
                    )?;
                    writeln!(
                        writer,
                        "performing function level profiling. Turned off by default"
                    )?;
                    writeln!(writer, "since CPU hit is non-zero.")?;
                }
                "clear" => {
                    writeln!(writer, "clear [pattern]")?;
                    writeln!(writer)?;
                    writeln!(writer, "Clears the profile inclusion/exclusion list.")?;
                    writeln!(
                        writer,
                        "If pattern is specified, only matching patterns are cleared."
                    )?;
                }
                "add" => {
                    writeln!(writer, "add (+|-) <pattern>")?;
                    writeln!(writer)?;
                    writeln!(writer, "Adds a pattern to the profile list. By default all")?;
                    writeln!(
                        writer,
                        "profile ranges are disabled. Each new range is then checked"
                    )?;
                    writeln!(
                        writer,
                        "against all pattern in this list. If a match is found the"
                    )?;
                    writeln!(
                        writer,
                        "active/inactive state is modified accordingly (+ for active,"
                    )?;
                    writeln!(
                        writer,
                        "- for inactive). The final state is always the last match."
                    )?;
                }
                "view" => {
                    writeln!(writer, "view")?;
                    writeln!(writer)?;
                    writeln!(writer, "Shows the active pattern list.")?;
                }
                _ => return Ok(false),
            }
        }
        Ok(true)
    }

    /// Handle the "result" command
    fn handle_result_command<W: Write>(
        writer: &mut W,
        normal_mode: bool,
        args: &[&str],
    ) -> ProfileResult<bool> {
        let state = &*COMMAND_STATE;

        if args.is_empty() {
            // List available result functions
            let factories = state.result_factories.lock().unwrap();
            for factory in factories.values() {
                write!(writer, "{}", factory.name)?;
                if (!factory.description.is_empty() && factory.description != factory.name)
                    || !normal_mode
                {
                    writeln!(writer, "  {}", factory.description)?;
                } else {
                    writeln!(writer)?;
                }
            }
        } else {
            // Add a result function
            let func_name = args[0];
            let func_args = &args[1..];

            let factories = state.result_factories.lock().unwrap();
            let factory = factories.get(func_name);

            match factory {
                Some(factory) => {
                    if let Some(result_fn) = (factory.factory_fn)(func_args) {
                        drop(factories); // Release lock before acquiring another
                        let mut result_functions =
                            state.registered_result_functions.lock().unwrap();
                        result_functions.push(result_fn);

                        if normal_mode {
                            writeln!(writer, "Result function {} added", func_name)?;
                        }
                    } else {
                        writeln!(writer, "Could not add result function")?;
                    }
                }
                None => {
                    writeln!(writer, "Unknown result function")?;
                }
            }
        }

        Ok(true)
    }

    /// Handle the "caller" command
    fn handle_caller_command<W: Write>(
        writer: &mut W,
        normal_mode: bool,
        args: &[&str],
    ) -> ProfileResult<bool> {
        #[cfg(feature = "function-level")]
        {
            let mut record_caller = Profile::func_level().is_caller_tracking_enabled();

            if !args.is_empty() {
                if args[0].starts_with('+') {
                    record_caller = true;
                    Profile::func_level().set_caller_tracking(true);
                } else if args[0].starts_with('-') {
                    record_caller = false;
                    Profile::func_level().set_caller_tracking(false);
                }
            }

            if normal_mode {
                writeln!(
                    writer,
                    "Record caller: {}",
                    if record_caller { "on" } else { "off" }
                )?;
            } else {
                writeln!(writer, "{}", if record_caller { "1" } else { "0" })?;
            }
        }

        #[cfg(not(feature = "function-level"))]
        {
            if normal_mode {
                writeln!(writer, "Function-level profiling not enabled")?;
            } else {
                writeln!(writer, "0")?;
            }
        }

        Ok(true)
    }

    /// Handle the "clear" command
    fn handle_clear_command<W: Write>(
        writer: &mut W,
        _normal_mode: bool,
        args: &[&str],
    ) -> ProfileResult<bool> {
        let pattern = args.first().unwrap_or(&"*");

        if *pattern == "*" {
            Profile::clear_patterns();
        } else {
            Profile::clear_patterns_matching(pattern);
        }

        Ok(true)
    }

    /// Handle the "add" command
    fn handle_add_command<W: Write>(
        writer: &mut W,
        _normal_mode: bool,
        args: &[&str],
    ) -> ProfileResult<bool> {
        if args.len() < 2 {
            writeln!(writer, "Please specify mode and pattern")?;
        } else {
            let mode = args[0];
            let pattern = args[1];

            let is_active = mode.starts_with('+');
            Profile::add_pattern(pattern, is_active)?;
        }

        Ok(true)
    }

    /// Handle the "view" command
    fn handle_view_command<W: Write>(
        writer: &mut W,
        _normal_mode: bool,
        _args: &[&str],
    ) -> ProfileResult<bool> {
        for (is_active, pattern) in Profile::get_patterns() {
            writeln!(writer, "{} {}", if is_active { "+" } else { "-" }, pattern)?;
        }
        Ok(true)
    }

    /// Register default result functions
    fn register_default_result_functions(state: &mut CommandInterfaceState) {
        let mut factories = state.result_factories.lock().unwrap();

        // CSV file writer
        factories.insert(
            "csv_file".to_string(),
            ResultFactory {
                name: "csv_file".to_string(),
                description:
                    "[ filename ] - Write results to CSV file (default: profile_results.csv)"
                        .to_string(),
                factory_fn: |args| ResultFunctionRegistry::create_function("csv_file", args),
            },
        );

        // C++-style CSV file writer
        factories.insert(
            "file_csv".to_string(),
            ResultFactory {
                name: "file_csv".to_string(),
                description: "Write results to C++-style CSV files".to_string(),
                factory_fn: |args| ResultFunctionRegistry::create_function("file_csv", args),
            },
        );

        // C++-style DOT call graph writer
        factories.insert(
            "file_dot".to_string(),
            ResultFactory {
                name: "file_dot".to_string(),
                description: "[ filename ] [ frame ] [ fold_threshold ] - Write DOT call graph"
                    .to_string(),
                factory_fn: |args| ResultFunctionRegistry::create_function("file_dot", args),
            },
        );

        // HTML file writer
        factories.insert(
            "html_file".to_string(),
            ResultFactory {
                name: "html_file".to_string(),
                description:
                    "[ filename ] - Write results to HTML file (default: profile_results.html)"
                        .to_string(),
                factory_fn: |args| ResultFunctionRegistry::create_function("html_file", args),
            },
        );

        // Console writer
        factories.insert(
            "console".to_string(),
            ResultFactory {
                name: "console".to_string(),
                description: "[ verbose ] - Write results to console".to_string(),
                factory_fn: |args| ResultFunctionRegistry::create_function("console", args),
            },
        );
    }
}

/// Extension trait for easier usage with standard output streams
pub trait ProfileCommandExecutor {
    /// Execute a profile command with this writer
    fn execute_profile_command(
        &mut self,
        cmd: &str,
        cmd_mode: CommandMode,
        args: &[&str],
    ) -> ProfileResult<bool>;
}

impl<W: Write> ProfileCommandExecutor for W {
    fn execute_profile_command(
        &mut self,
        cmd: &str,
        cmd_mode: CommandMode,
        args: &[&str],
    ) -> ProfileResult<bool> {
        ProfileCmdInterface::execute(self, cmd, cmd_mode, args)
    }
}

/// Convenience function for executing commands with stdout
pub fn execute_command_with_stdout(
    cmd: &str,
    cmd_mode: CommandMode,
    args: &[&str],
) -> ProfileResult<bool> {
    let mut stdout = io::stdout();
    ProfileCmdInterface::execute(&mut stdout, cmd, cmd_mode, args)
}

/// Convenience function for executing commands and returning output as string
pub fn execute_command_to_string(
    cmd: &str,
    cmd_mode: CommandMode,
    args: &[&str],
) -> ProfileResult<(bool, String)> {
    let mut output = Vec::new();
    let handled = ProfileCmdInterface::execute(&mut output, cmd, cmd_mode, args)?;
    let output_str = String::from_utf8(output)
        .map_err(|_| ProfileError::PatternError("Invalid UTF-8 in command output".to_string()))?;
    Ok((handled, output_str))
}

/// Simple command-line parser for profile commands
pub struct ProfileCommandParser;

impl ProfileCommandParser {
    /// Parse a command line string into command and arguments
    ///
    /// # Arguments
    /// * `line` - Command line string (e.g., "result csv_file profile.csv")
    ///
    /// # Returns
    /// (command, arguments) tuple
    pub fn parse_command_line(line: &str) -> (&str, Vec<&str>) {
        let mut parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            return ("", vec![]);
        }

        let command = parts[0];
        parts.remove(0);
        (command, parts)
    }

    /// Execute a command line string
    ///
    /// # Arguments
    /// * `writer` - Output writer
    /// * `line` - Full command line
    /// * `cmd_mode` - Command mode
    ///
    /// # Returns
    /// `true` if command was handled, `false` if unknown
    pub fn execute_command_line<W: Write>(
        writer: &mut W,
        line: &str,
        cmd_mode: CommandMode,
    ) -> ProfileResult<bool> {
        let (cmd, args) = Self::parse_command_line(line);
        if cmd.is_empty() {
            return Ok(false);
        }

        ProfileCmdInterface::execute(writer, cmd, cmd_mode, &args)
    }
}

// Convert io::Error to ProfileError
impl From<io::Error> for ProfileError {
    fn from(err: io::Error) -> Self {
        ProfileError::PatternError(format!("IO Error: {}", err))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_help_command() {
        let mut output = Vec::new();
        let result = ProfileCmdInterface::execute(&mut output, "help", CommandMode::Normal, &[]);

        assert!(result.is_ok());
        assert!(result.unwrap());

        let output_str = String::from_utf8(output).unwrap();
        assert!(output_str.contains("profile group help"));
        assert!(output_str.contains("result, caller, clear, add, view"));
    }

    #[test]
    fn test_help_result_command() {
        let mut output = Vec::new();
        let result =
            ProfileCmdInterface::execute(&mut output, "help", CommandMode::Normal, &["result"]);

        assert!(result.is_ok());
        assert!(result.unwrap());

        let output_str = String::from_utf8(output).unwrap();
        assert!(output_str.contains("result"));
        assert!(output_str.contains("res_func_name"));
    }

    #[test]
    fn test_result_command_list() {
        let mut output = Vec::new();
        let result = ProfileCmdInterface::execute(&mut output, "result", CommandMode::Normal, &[]);

        assert!(result.is_ok());
        assert!(result.unwrap());

        let output_str = String::from_utf8(output).unwrap();
        assert!(output_str.contains("csv_file"));
        assert!(output_str.contains("html_file"));
        assert!(output_str.contains("console"));
    }

    #[test]
    fn test_result_command_add() {
        let mut output = Vec::new();
        let result = ProfileCmdInterface::execute(
            &mut output,
            "result",
            CommandMode::Normal,
            &["csv_file", "test_output.csv"],
        );

        assert!(result.is_ok());
        assert!(result.unwrap());

        let output_str = String::from_utf8(output).unwrap();
        assert!(output_str.contains("Result function csv_file added"));
    }

    #[test]
    fn test_result_command_unknown() {
        let mut output = Vec::new();
        let result = ProfileCmdInterface::execute(
            &mut output,
            "result",
            CommandMode::Normal,
            &["unknown_function"],
        );

        assert!(result.is_ok());
        assert!(result.unwrap());

        let output_str = String::from_utf8(output).unwrap();
        assert!(output_str.contains("Unknown result function"));
    }

    #[test]
    fn test_add_command() {
        let mut output = Vec::new();
        let result =
            ProfileCmdInterface::execute(&mut output, "add", CommandMode::Normal, &["+", "test.*"]);

        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_add_command_insufficient_args() {
        let mut output = Vec::new();
        let result = ProfileCmdInterface::execute(&mut output, "add", CommandMode::Normal, &["+"]);

        assert!(result.is_ok());
        assert!(result.unwrap());

        let output_str = String::from_utf8(output).unwrap();
        assert!(output_str.contains("Please specify mode and pattern"));
    }

    #[test]
    fn test_unknown_command() {
        let mut output = Vec::new();
        let result = ProfileCmdInterface::execute(&mut output, "unknown", CommandMode::Normal, &[]);

        assert!(result.is_ok());
        assert!(!result.unwrap()); // Should return false for unknown commands
    }

    #[test]
    fn test_command_mode_script() {
        let mut output = Vec::new();
        let result = ProfileCmdInterface::execute(&mut output, "help", CommandMode::Script, &[]);

        assert!(result.is_ok());
        assert!(result.unwrap());

        // In script mode, help should produce no output
        let output_str = String::from_utf8(output).unwrap();
        assert!(output_str.is_empty());
    }

    #[test]
    fn test_command_parser() {
        let (cmd, args) = ProfileCommandParser::parse_command_line("result csv_file output.csv");
        assert_eq!(cmd, "result");
        assert_eq!(args, vec!["csv_file", "output.csv"]);

        let (cmd, args) = ProfileCommandParser::parse_command_line("help");
        assert_eq!(cmd, "help");
        assert!(args.is_empty());

        let (cmd, args) = ProfileCommandParser::parse_command_line("");
        assert_eq!(cmd, "");
        assert!(args.is_empty());
    }

    #[test]
    fn test_execute_command_line() {
        let mut output = Vec::new();
        let result = ProfileCommandParser::execute_command_line(
            &mut output,
            "help result",
            CommandMode::Normal,
        );

        assert!(result.is_ok());
        assert!(result.unwrap());

        let output_str = String::from_utf8(output).unwrap();
        assert!(output_str.contains("result"));
        assert!(output_str.contains("res_func_name"));
    }

    #[test]
    fn test_execute_command_to_string() {
        let result = execute_command_to_string("help", CommandMode::Normal, &[]);
        assert!(result.is_ok());

        let (handled, output) = result.unwrap();
        assert!(handled);
        assert!(output.contains("profile group help"));
    }

    #[test]
    fn test_profile_command_executor_trait() {
        let mut cursor = Cursor::new(Vec::new());
        let result = cursor.execute_profile_command("help", CommandMode::Normal, &[]);

        assert!(result.is_ok());
        assert!(result.unwrap());

        let output = cursor.into_inner();
        let output_str = String::from_utf8(output).unwrap();
        assert!(output_str.contains("profile group help"));
    }
}
