#![allow(dead_code)]

mod app;
mod legacy;
mod model;

pub fn run() -> anyhow::Result<()> {
    app::run()
}
