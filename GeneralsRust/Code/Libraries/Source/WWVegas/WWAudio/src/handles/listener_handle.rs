use std::sync::{Arc, Mutex};

use crate::{
    error::{Error, Result},
    listener::Listener3D,
    math::{Matrix3D, Vector3},
};

/// Listener handle analogue (`ListenerHandleClass`)
pub struct ListenerHandle {
    listener: Arc<Mutex<Listener3D>>,
}

impl ListenerHandle {
    pub fn new(listener: Listener3D) -> Self {
        Self {
            listener: Arc::new(Mutex::new(listener)),
        }
    }

    pub fn with_shared(listener: Arc<Mutex<Listener3D>>) -> Self {
        Self { listener }
    }

    pub fn listener(&self) -> Arc<Mutex<Listener3D>> {
        Arc::clone(&self.listener)
    }

    pub fn set_position(&self, position: Vector3) -> Result<()> {
        let mut listener = self
            .listener
            .lock()
            .map_err(|_| Error::Audio("Listener lock poisoned".to_string()))?;
        listener.set_position(position);
        Ok(())
    }

    pub fn position(&self) -> Result<Vector3> {
        let listener = self
            .listener
            .lock()
            .map_err(|_| Error::Audio("Listener lock poisoned".to_string()))?;
        Ok(listener.position())
    }

    pub fn set_transform(&self, transform: Matrix3D) -> Result<()> {
        let mut listener = self
            .listener
            .lock()
            .map_err(|_| Error::Audio("Listener lock poisoned".to_string()))?;
        listener.set_transform(transform);
        Ok(())
    }

    pub fn transform(&self) -> Result<Matrix3D> {
        let listener = self
            .listener
            .lock()
            .map_err(|_| Error::Audio("Listener lock poisoned".to_string()))?;
        Ok(listener.transform())
    }
}

impl Clone for ListenerHandle {
    fn clone(&self) -> Self {
        Self {
            listener: Arc::clone(&self.listener),
        }
    }
}
