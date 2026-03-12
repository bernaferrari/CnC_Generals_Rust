use crate::debug_debug::Debug;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StringType {
    Assert = 0,
    Check = 1,
    Log = 2,
    Crash = 3,
    Exception = 4,
    CmdReply = 5,
    StructuredCmdReply = 6,
    Other = 7,
}

pub trait DebugIOInterface: Send {
    fn read(&mut self, buf: &mut [u8]) -> usize;
    fn write(&mut self, kind: StringType, src: Option<&str>, message: Option<&str>);
    fn emergency_flush(&mut self);
    fn execute(&mut self, dbg: &mut Debug, cmd: &str, structured: bool, argv: &[&str]);
    fn delete(self: Box<Self>);
}

pub type IOFactory = fn() -> Box<dyn DebugIOInterface>;
