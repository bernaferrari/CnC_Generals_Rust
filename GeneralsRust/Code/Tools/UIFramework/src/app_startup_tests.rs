//! Tests the actual native app-creation boundary without opening a window.
use super::*;
use std::cell::Cell;
use std::rc::Rc;
use uuid::Uuid;

#[derive(Default)]
struct Calls {
    initialized: Cell<usize>,
    updated: Cell<usize>,
    shutdown: Cell<usize>,
}

struct Tool {
    config: ToolConfig,
    calls: Rc<Calls>,
    fail: bool,
}

impl GameTool for Tool {
    fn id(&self) -> Uuid {
        Uuid::nil()
    }
    fn name(&self) -> &str {
        "StartupFixture"
    }
    fn version(&self) -> &str {
        "1"
    }
    fn config(&self) -> &ToolConfig {
        &self.config
    }
    fn set_config(&mut self, config: ToolConfig) -> Result<()> {
        self.config = config;
        Ok(())
    }
    fn initialize(&mut self) -> Result<()> {
        self.calls.initialized.set(self.calls.initialized.get() + 1);
        if self.fail {
            anyhow::bail!("fixture startup failure");
        }
        Ok(())
    }
    fn update(&mut self, _: &mut egui::Ui, _: &mut eframe::Frame) -> Result<()> {
        self.calls.updated.set(self.calls.updated.get() + 1);
        Ok(())
    }
    fn menu_bar(&mut self, _: &mut egui::Ui) -> Result<()> {
        Ok(())
    }
    fn shutdown(&mut self) -> Result<()> {
        self.calls.shutdown.set(self.calls.shutdown.get() + 1);
        Ok(())
    }
}

fn create(fail: bool, calls: Rc<Calls>) -> ToolApp {
    ToolApp::new(Box::new(Tool {
        config: ToolConfig {
            hot_reload_enabled: false,
            ..ToolConfig::default()
        },
        calls,
        fail,
    }))
    .unwrap()
}

#[test]
fn failed_initialization_cannot_produce_a_running_app() {
    let calls = Rc::new(Calls::default());
    let app = create(true, calls.clone());
    assert_eq!(calls.initialized.get(), 0);
    let result = app.initialize_for_run(&egui::Context::default());
    assert!(result.is_err());
    assert_eq!(result.err().unwrap().to_string(), "fixture startup failure");
    assert_eq!(calls.initialized.get(), 1);
    assert_eq!(calls.updated.get(), 0);
    assert_eq!(calls.shutdown.get(), 0);
}

#[test]
fn successful_initialization_is_called_once_after_inert_construction() {
    let calls = Rc::new(Calls::default());
    let app = create(false, calls.clone());
    assert_eq!(calls.initialized.get(), 0);
    let app = app.initialize_for_run(&egui::Context::default()).unwrap();
    assert_eq!(calls.initialized.get(), 1);
    assert_eq!(calls.updated.get(), 0);
    assert_eq!(calls.shutdown.get(), 0);
    assert_eq!(app.tool.name(), "StartupFixture");
}
