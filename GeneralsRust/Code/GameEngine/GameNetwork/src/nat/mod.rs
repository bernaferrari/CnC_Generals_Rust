//! NAT traversal module providing STUN, UPnP, and related functionality.
//!
//! This module provides comprehensive NAT traversal capabilities including:
//! - STUN for public address discovery and NAT type detection
//! - UPnP for automatic port forwarding
//! - Integrated NAT service combining both approaches

pub mod nat;
pub mod service;
pub mod stun;
pub mod upnp;

// Re-export service types (existing NAT functionality)
pub use service::{NatBinding, NatConfig, NatService};

// Re-export STUN types
pub use stun::{StunClient, StunConfig, StunNatType};

// Re-export UPnP types
pub use upnp::{PortMapping, UPnPClient, UPnPConfig, UPnPGateway};
