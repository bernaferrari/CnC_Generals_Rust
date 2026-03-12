use crate::debug_debug::Debug;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandMode {
    Normal,
    Structured,
}

pub trait DebugCmdInterface: Send {
    fn execute(&mut self, dbg: &mut Debug, cmd: &str, mode: CommandMode, argv: &[&str]) -> bool;

    fn delete(self: Box<Self>);
}

pub struct DebugCmdInterfaceDebug;

impl DebugCmdInterfaceDebug {
    pub fn new() -> Self {
        Self
    }
}

impl DebugCmdInterface for DebugCmdInterfaceDebug {
    fn execute(&mut self, dbg: &mut Debug, cmd: &str, mode: CommandMode, argv: &[&str]) -> bool {
        let normal_mode = mode == CommandMode::Normal;

        match cmd {
            "help" => {
                if !normal_mode {
                    return true;
                }
                if argv.is_empty() {
                    dbg.write_plain(
                        "debug group help:\n  list, io, alwaysflush, timestamp, exit, clear, add, view\n",
                    );
                    return true;
                }
                match argv[0] {
                    "list" => {
                        dbg.write_plain(
                            "list (g|l|d|a|c) [ <pattern> ]\n\nShows items by type:\n- g: command groups\n- l: log groups\n- d: log groups w/ descriptions\n- a: asserts/crashes\n- c: checks\n",
                        );
                        true
                    }
                    "io" => {
                        dbg.write_plain(
                            "io <I/O Class> <cmd> { <param> }\n\nIf no params: list active I/O. Use 'io ?' to list possible classes.\n",
                        );
                        true
                    }
                    "alwaysflush" => {
                        dbg.write_plain(
                            "alwaysflush [ (+|-) ]\n\nEnables/disables flushing after each entry.\n",
                        );
                        true
                    }
                    "timestamp" => {
                        dbg.write_plain(
                            "timestamp [ (+|-) ]\n\nEnables/disables timestamping each entry.\n",
                        );
                        true
                    }
                    "exit" => {
                        dbg.write_plain("exit\n\nExits program immediately.\n");
                        true
                    }
                    "clear" => {
                        dbg.write_plain("clear (l|a|c)\n\nClears pattern list.\n");
                        true
                    }
                    "add" => {
                        dbg.write_plain(
                            "add (l|a|c) (+|-) <pattern>\n\nAdds a pattern for logs/asserts/checks.\n",
                        );
                        true
                    }
                    "view" => {
                        dbg.write_plain("view [ (l|a|c) ]\n\nShows active patterns.\n");
                        true
                    }
                    _ => false,
                }
            }
            "list" => {
                let pattern = argv.get(1).copied().unwrap_or("*");
                match argv.get(0).copied().unwrap_or("") {
                    "g" => {
                        if normal_mode {
                            dbg.write_plain("Command groups:\n");
                        }
                        for group in dbg.list_cmd_groups() {
                            if crate::debug_debug::simple_match(&group, pattern) {
                                dbg.write_plain(&format!("{group}\n"));
                            }
                        }
                    }
                    "l" | "d" => {
                        if normal_mode {
                            dbg.write_plain("Logs:\n");
                        }
                        for group in dbg.list_log_groups() {
                            if crate::debug_debug::simple_match(&group.name, pattern) {
                                if argv.get(0).copied() == Some("d") && group.description.is_none()
                                {
                                    continue;
                                }
                                let mut line = group.name.clone();
                                if let Some(desc) = &group.description {
                                    line.push_str(" (");
                                    line.push_str(desc);
                                    line.push(')');
                                }
                                line.push('\n');
                                dbg.write_plain(&line);
                            }
                        }
                    }
                    "a" | "c" => {
                        let kind = if argv.get(0).copied() == Some("a") {
                            crate::debug_debug::DebugPatternType::Assert
                        } else {
                            crate::debug_debug::DebugPatternType::Check
                        };
                        if normal_mode {
                            dbg.write_plain(
                                if kind == crate::debug_debug::DebugPatternType::Assert {
                                    "Asserts/Crashes:\n"
                                } else {
                                    "Checks:\n"
                                },
                            );
                        }
                        for (active, pat) in dbg.list_patterns(kind) {
                            if crate::debug_debug::simple_match(&pat, pattern) {
                                let status = if active { "" } else { " [off]" };
                                dbg.write_plain(&format!("{pat}{status}\n"));
                            }
                        }
                    }
                    _ => {
                        dbg.write_plain("Unknown item type, see help.\n");
                    }
                }
                true
            }
            "io" => {
                if argv.is_empty() || argv[0] == "?" {
                    let active_only = argv.is_empty();
                    let heading = if active_only {
                        "Active:\n"
                    } else {
                        "Possible:\n"
                    };
                    dbg.write_plain(heading);
                    for (id, descr) in dbg.list_io(active_only) {
                        dbg.write_plain(&format!("{id} ({descr})\n"));
                    }
                    return true;
                }

                let id = argv[0];
                if argv.len() == 1 {
                    dbg.write_plain("Missing I/O command.\n");
                    return true;
                }

                if argv[1] == "remove" {
                    dbg.detach_io(id);
                    return true;
                }

                if argv[1] == "add" {
                    dbg.attach_io(id);
                    if argv.len() == 2 {
                        return true;
                    }
                }

                if !dbg.io_execute(id, argv[1], mode == CommandMode::Structured, &argv[2..]) {
                    dbg.write_plain(&format!("Unknown I/O class {id}\n"));
                }
                true
            }
            "alwaysflush" => {
                if argv.is_empty() {
                    dbg.write_plain("alwaysflush requires +/-\n");
                    return true;
                }
                let enabled = argv[0] == "+" || argv[0] == "1" || argv[0] == "true";
                dbg.toggle_always_flush(enabled);
                true
            }
            "timestamp" => {
                if argv.is_empty() {
                    dbg.write_plain("timestamp requires +/-\n");
                    return true;
                }
                let enabled = argv[0] == "+" || argv[0] == "1" || argv[0] == "true";
                dbg.toggle_timestamp(enabled);
                true
            }
            "exit" => {
                std::process::exit(1);
            }
            "clear" => {
                if argv.is_empty() {
                    return true;
                }
                match argv[0] {
                    "l" => dbg.clear_patterns(crate::debug_debug::DebugPatternType::Log),
                    "a" => dbg.clear_patterns(crate::debug_debug::DebugPatternType::Assert),
                    "c" => dbg.clear_patterns(crate::debug_debug::DebugPatternType::Check),
                    _ => {}
                }
                true
            }
            "add" => {
                if argv.len() < 3 {
                    return true;
                }
                let list_type = argv[0];
                let active = argv[1] == "+";
                let pattern = argv[2];
                match list_type {
                    "l" => {
                        dbg.add_pattern(crate::debug_debug::DebugPatternType::Log, active, pattern)
                    }
                    "a" => dbg.add_pattern(
                        crate::debug_debug::DebugPatternType::Assert,
                        active,
                        pattern,
                    ),
                    "c" => dbg.add_pattern(
                        crate::debug_debug::DebugPatternType::Check,
                        active,
                        pattern,
                    ),
                    _ => {}
                }
                true
            }
            "view" => {
                let list_type = argv.get(0).copied();
                let types = match list_type {
                    Some("l") => vec![crate::debug_debug::DebugPatternType::Log],
                    Some("a") => vec![crate::debug_debug::DebugPatternType::Assert],
                    Some("c") => vec![crate::debug_debug::DebugPatternType::Check],
                    _ => vec![
                        crate::debug_debug::DebugPatternType::Log,
                        crate::debug_debug::DebugPatternType::Assert,
                        crate::debug_debug::DebugPatternType::Check,
                    ],
                };
                for ty in types {
                    let header = match ty {
                        crate::debug_debug::DebugPatternType::Log => "Log patterns:\n",
                        crate::debug_debug::DebugPatternType::Assert => "Assert patterns:\n",
                        crate::debug_debug::DebugPatternType::Check => "Check patterns:\n",
                    };
                    dbg.write_plain(header);
                    for (active, pat) in dbg.list_patterns(ty) {
                        let status = if active { "+" } else { "-" };
                        dbg.write_plain(&format!("{status} {pat}\n"));
                    }
                }
                true
            }
            _ => false,
        }
    }

    fn delete(self: Box<Self>) {}
}

impl<T> DebugCmdInterface for T
where
    T: Send + 'static + FnMut(&mut Debug, &str, CommandMode, &[&str]) -> bool,
{
    fn execute(&mut self, dbg: &mut Debug, cmd: &str, mode: CommandMode, argv: &[&str]) -> bool {
        (self)(dbg, cmd, mode, argv)
    }

    fn delete(self: Box<Self>) {}
}
