//! Session-owned GameData capacity for cave and player tunnel trackers.
use game_engine::common::ini::ini_game_data::TunnelCapacityRules;

impl super::GameLogic {
    pub(crate) fn effective_tunnel_capacity(&self) -> usize {
        usize::try_from(self.cave_system_residual().capacity()).unwrap_or(0)
    }

    fn install_tunnel_capacity(&mut self, capacity: i32) {
        self.cave_system_residual_mut().set_capacity(capacity);
        self.tunnel_network.set_capacity(capacity);
    }

    pub(crate) fn admit_tunnel_capacity_baseline(&mut self, capacity: i32) {
        self.tunnel_capacity_base = capacity;
        self.install_tunnel_capacity(capacity);
    }

    pub(crate) fn restore_tunnel_capacity_baseline(&mut self) {
        self.install_tunnel_capacity(self.tunnel_capacity_base);
    }

    /// Admit ordered application sources before loading a world.
    pub fn admit_tunnel_capacity_ini_sources<I, S>(&mut self, sources: I) -> Result<(), String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        if !self.objects.registered_is_empty() {
            return Err("tunnel capacity admission requires an empty receiving world".into());
        }
        let mut rules = TunnelCapacityRules::default();
        for source in sources {
            rules = rules.overlay_ini_text(source.as_ref())?;
        }
        self.admit_tunnel_capacity_baseline(rules.max_capacity);
        Ok(())
    }

    pub(crate) fn admit_tunnel_capacity_companion_sources(
        &mut self,
        sources: &[String],
    ) -> Result<(), String> {
        let mut rules = TunnelCapacityRules {
            max_capacity: self.tunnel_capacity_base,
        };
        for source in sources {
            rules = rules.overlay_ini_text(source)?;
        }
        self.install_tunnel_capacity(rules.max_capacity);
        Ok(())
    }
}
