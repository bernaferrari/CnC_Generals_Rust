//! GUI Menu Systems
//!
//! This module contains all the game menu implementations including
//! main menu, game setup menus, network connection menus, and more.

pub mod disconnect_menu;
pub mod establish_connections_menu;

// Re-export key types for convenience
pub use disconnect_menu::{DisconnectMenu, get_disconnect_menu};
pub use establish_connections_menu::{
    EstablishConnectionsMenu, EstablishConnectionsMenuState, NATConnectionState,
    get_establish_connections_menu,
};
