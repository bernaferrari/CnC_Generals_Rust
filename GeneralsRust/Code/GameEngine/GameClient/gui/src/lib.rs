#![allow(dead_code)]

mod app;
mod gui;
mod legacy;
mod model;
mod runtime_menu;

pub fn run() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    if let Some(flag) = args.next() {
        if flag == "--runtime-menu-ipc" {
            let ipc_path = args.next().ok_or_else(|| {
                anyhow::anyhow!("--runtime-menu-ipc requires a writable file path argument")
            })?;
            return runtime_menu::run_runtime_menu(ipc_path.into());
        }
    }

    app::run()
}
