//! Ordered permanent waypoint reveals from C++ ScriptEngine.cpp:6145-6241.

/// Named reveal structure matching C++ NamedReveal
#[derive(Debug, Clone)]
pub struct NamedReveal {
    pub reveal_name: String,
    pub waypoint_name: String,
    pub radius_to_reveal: f32,
    pub player_name: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NamedRevealOperation {
    Reveal,
    Undo,
}

#[derive(Default)]
pub struct NamedReveals {
    entries: Vec<NamedReveal>,
}

impl NamedReveals {
    /// The first definition keeps its parameters for the lifetime of the name.
    pub fn register(&mut self, name: &str, waypoint: &str, radius: f32, player: &str) -> bool {
        if self.find(name).is_some() {
            return false;
        }
        self.entries.push(NamedReveal {
            reveal_name: name.to_owned(),
            waypoint_name: waypoint.to_owned(),
            radius_to_reveal: radius,
            player_name: player.to_owned(),
        });
        true
    }

    pub fn find(&self, name: &str) -> Option<&NamedReveal> {
        self.entries.iter().find(|entry| entry.reveal_name == name)
    }

    pub fn remove(&mut self, name: &str) {
        if let Some(index) = self
            .entries
            .iter()
            .position(|entry| entry.reveal_name == name)
        {
            self.entries.remove(index);
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn entries(&self) -> &[NamedReveal] {
        &self.entries
    }

    /// Xfer admits each row in wire order, including duplicate names.
    pub fn legacy_entries_mut(&mut self) -> &mut Vec<NamedReveal> {
        &mut self.entries
    }

    pub fn snapshot(&self) -> Vec<(String, String, f32, String)> {
        self.entries
            .iter()
            .map(|entry| {
                (
                    entry.reveal_name.clone(),
                    entry.waypoint_name.clone(),
                    entry.radius_to_reveal,
                    entry.player_name.clone(),
                )
            })
            .collect()
    }

    pub fn restore(&mut self, rows: &[(String, String, f32, String)]) {
        self.entries.clear();
        self.entries.extend(
            rows.iter()
                .map(|(name, waypoint, radius, player)| NamedReveal {
                    reveal_name: name.clone(),
                    waypoint_name: waypoint.clone(),
                    radius_to_reveal: *radius,
                    player_name: player.clone(),
                }),
        );
    }
}
