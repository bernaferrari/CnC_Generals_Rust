use crate::debug_debug::Debug;
use backtrace::{Backtrace, Symbol};

#[derive(Clone, Debug)]
pub struct Signature {
    addresses: Vec<usize>,
}

impl Signature {
    pub fn new() -> Self {
        Self {
            addresses: Vec::new(),
        }
    }

    pub fn size(&self) -> usize {
        self.addresses.len()
    }

    pub fn get_address(&self, index: usize) -> Option<usize> {
        self.addresses.get(index).copied()
    }

    pub fn get_symbol(addr: usize) -> String {
        let mut module = String::from("unknown");
        let mut symbol_name = String::from("unknown");
        let mut file = String::from("unknown");
        let mut line = 0u32;
        let mut rel_sym = 0usize;

        backtrace::resolve(addr as *mut _, |symbol| {
            fill_symbol(
                symbol,
                &mut module,
                &mut symbol_name,
                &mut file,
                &mut line,
                &mut rel_sym,
                addr,
            );
        });

        format!(
            "{:08x} {}+0x{:x}, {}+0x{:x}, {}:{}+0x0",
            addr, module, 0, symbol_name, rel_sym, file, line
        )
    }
}

fn fill_symbol(
    symbol: &Symbol,
    module: &mut String,
    symbol_name: &mut String,
    file: &mut String,
    line: &mut u32,
    rel_sym: &mut usize,
    addr: usize,
) {
    if let Some(name) = symbol.name() {
        *symbol_name = format!("{}", name);
    }

    if let Some(filename) = symbol.filename() {
        if let Some(name) = filename.file_name() {
            *file = name.to_string_lossy().to_string();
        } else {
            *file = filename.to_string_lossy().to_string();
        }
    }

    if let Some(lineno) = symbol.lineno() {
        *line = lineno as u32;
    }

    if let Some(sym_addr) = symbol.addr() {
        let sym_addr = sym_addr as usize;
        if addr >= sym_addr {
            *rel_sym = addr - sym_addr;
        }
    }

    let _ = module;
}

pub struct DebugStackwalk;

impl DebugStackwalk {
    pub fn get_dbghelp_handle() -> Option<usize> {
        None
    }

    pub fn is_old_dbghelp() -> bool {
        false
    }

    pub fn stack_walk(signature: &mut Signature) -> usize {
        signature.addresses.clear();

        let bt = Backtrace::new_unresolved();
        for frame in bt.frames() {
            let ip = frame.ip() as usize;
            if ip == 0 {
                continue;
            }
            signature.addresses.push(ip);
            if signature.addresses.len() >= 256 {
                break;
            }
        }

        signature.addresses.len()
    }
}

pub fn write_signature(dbg: &mut Debug, sig: &Signature) {
    dbg.write_plain(&format!("{} addresses:\n", sig.size()));
    for (idx, addr) in sig.addresses.iter().enumerate() {
        let line = format!("{:3} {}\n", idx, Signature::get_symbol(*addr));
        dbg.write_plain(&line);
    }
}
