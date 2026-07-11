// FILE: w3_d_thing_factory.rs
// Ported from C++ W3DThingFactory.h/.cpp

use game_engine::common::thing::ThingFactory;

/// Device-dependent thing factory access for W3D.
///
/// The original C++ constructor and destructor are empty; this wrapper preserves
/// the type boundary while delegating normal thing creation to `ThingFactory`.
pub struct W3DThingFactory {
    base: ThingFactory,
}

impl W3DThingFactory {
    /// Creates a W3D thing factory with a fresh base `ThingFactory`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            base: ThingFactory::new(),
        }
    }

    /// Returns the underlying shared thing factory.
    #[must_use]
    pub fn base(&self) -> &ThingFactory {
        &self.base
    }

    /// Returns the underlying shared thing factory mutably.
    pub fn base_mut(&mut self) -> &mut ThingFactory {
        &mut self.base
    }
}

impl Default for W3DThingFactory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn w3d_thing_factory_constructs_like_cpp_empty_ctor() {
        let _factory = W3DThingFactory::new();
    }
}
