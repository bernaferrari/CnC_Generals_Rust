#![allow(dead_code)]

mod app;
mod gui;
mod legacy;
mod model;

pub fn run() -> anyhow::Result<()> {
    app::run()
}
