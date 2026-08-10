//! Shell-smoke status report formatting.

use super::result::ShellSmokeResult;

pub fn format_shell_smoke_report(r: &ShellSmokeResult) -> String {
    format!("shell_smoke status={} detail={}", r.status, r.detail)
}
