//! Windows Firewall integration using COM API (INetFwPolicy2).
//!
//! This module provides cross-platform firewall integration with
//! Windows-specific support via COM interfaces.

use crate::error::NetworkResult;
use tracing::debug;

/// Cross-platform firewall integration trait.
pub trait FirewallIntegration {
    /// Add a port exception to the firewall.
    fn add_port_exception(&self, port: u16, protocol: Protocol) -> NetworkResult<()>;

    /// Remove a port exception from the firewall.
    fn remove_port_exception(&self, port: u16, protocol: Protocol) -> NetworkResult<()>;

    /// Check if a port has an exception.
    fn has_port_exception(&self, port: u16, protocol: Protocol) -> NetworkResult<bool>;
}

/// Network protocol for firewall rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    Tcp,
    Udp,
}

impl Protocol {
    pub fn as_str(&self) -> &'static str {
        match self {
            Protocol::Tcp => "TCP",
            Protocol::Udp => "UDP",
        }
    }

    #[cfg(target_os = "windows")]
    pub fn as_net_fw_protocol(&self) -> i32 {
        match self {
            Protocol::Tcp => 6,  // NET_FW_IP_PROTOCOL_TCP
            Protocol::Udp => 17, // NET_FW_IP_PROTOCOL_UDP
        }
    }
}

/// Windows Firewall helper using COM API.
#[cfg(target_os = "windows")]
pub struct WindowsFirewallHelper {
    rule_name_prefix: String,
}

#[cfg(target_os = "windows")]
impl WindowsFirewallHelper {
    /// Create a new Windows Firewall helper.
    pub fn new(app_name: &str) -> NetworkResult<Self> {
        Ok(Self {
            rule_name_prefix: format!("{} - Game Port", app_name),
        })
    }

    fn get_rule_name(&self, port: u16, protocol: Protocol) -> String {
        format!("{} {} {}", self.rule_name_prefix, port, protocol.as_str())
    }
}

#[cfg(target_os = "windows")]
impl FirewallIntegration for WindowsFirewallHelper {
    fn add_port_exception(&self, port: u16, protocol: Protocol) -> NetworkResult<()> {
        use windows::core::{ComInterface, BSTR};
        use windows::Win32::NetworkManagement::WindowsFirewall::{
            INetFwPolicy2, INetFwRule, NetFwPolicy2, NET_FW_ACTION_ALLOW, NET_FW_RULE_DIR_IN,
        };
        use windows::Win32::System::Com::{
            CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_APARTMENTTHREADED,
        };

        unsafe {
            // Initialize COM
            if let Err(e) = CoInitializeEx(None, COINIT_APARTMENTTHREADED) {
                // S_FALSE means already initialized, which is okay
                if e.code().0 != 0x00000001 {
                    return Err(NetworkError::generic(format!(
                        "Failed to initialize COM: {}",
                        e
                    )));
                }
            }

            // Create firewall policy object
            let policy: INetFwPolicy2 =
                CoCreateInstance(&NetFwPolicy2, None, CLSCTX_ALL).map_err(|e| {
                    NetworkError::generic(format!("Failed to create firewall policy: {}", e))
                })?;

            // Check if rule already exists
            let rule_name = self.get_rule_name(port, protocol);
            let rules = policy.Rules().map_err(|e| {
                NetworkError::generic(format!("Failed to get firewall rules: {}", e))
            })?;

            let rule_name_bstr = BSTR::from(rule_name.as_str());
            match rules.Item(&rule_name_bstr) {
                Ok(_) => {
                    debug!(
                        "Firewall rule already exists for {} port {}",
                        protocol.as_str(),
                        port
                    );
                    return Ok(());
                }
                Err(_) => {
                    // Rule doesn't exist, create it
                }
            }

            // Create new firewall rule
            let rule: INetFwRule = CoCreateInstance(
                &windows::Win32::NetworkManagement::WindowsFirewall::NetFwRule,
                None,
                CLSCTX_ALL,
            )
            .map_err(|e| NetworkError::generic(format!("Failed to create firewall rule: {}", e)))?;

            // Configure rule
            rule.SetName(&rule_name_bstr)
                .map_err(|e| NetworkError::generic(format!("Failed to set rule name: {}", e)))?;

            let description = BSTR::from(format!(
                "Allows incoming connections for C&C Generals Zero Hour on {} port {}",
                protocol.as_str(),
                port
            ));
            rule.SetDescription(&description).map_err(|e| {
                NetworkError::generic(format!("Failed to set rule description: {}", e))
            })?;

            rule.SetProtocol(protocol.as_net_fw_protocol())
                .map_err(|e| NetworkError::generic(format!("Failed to set protocol: {}", e)))?;

            let port_str = BSTR::from(port.to_string());
            rule.SetLocalPorts(&port_str)
                .map_err(|e| NetworkError::generic(format!("Failed to set local ports: {}", e)))?;

            rule.SetDirection(NET_FW_RULE_DIR_IN)
                .map_err(|e| NetworkError::generic(format!("Failed to set direction: {}", e)))?;

            rule.SetAction(NET_FW_ACTION_ALLOW)
                .map_err(|e| NetworkError::generic(format!("Failed to set action: {}", e)))?;

            rule.SetEnabled(true)
                .map_err(|e| NetworkError::generic(format!("Failed to enable rule: {}", e)))?;

            // Add rule to policy
            rules.Add(&rule).map_err(|e| {
                NetworkError::generic(format!("Failed to add rule to policy: {}", e))
            })?;

            info!(
                "Added Windows Firewall exception for {} port {}",
                protocol.as_str(),
                port
            );
            Ok(())
        }
    }

    fn remove_port_exception(&self, port: u16, protocol: Protocol) -> NetworkResult<()> {
        use windows::core::BSTR;
        use windows::Win32::NetworkManagement::WindowsFirewall::{INetFwPolicy2, NetFwPolicy2};
        use windows::Win32::System::Com::{
            CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_APARTMENTTHREADED,
        };

        unsafe {
            // Initialize COM
            if let Err(e) = CoInitializeEx(None, COINIT_APARTMENTTHREADED) {
                if e.code().0 != 0x00000001 {
                    return Err(NetworkError::generic(format!(
                        "Failed to initialize COM: {}",
                        e
                    )));
                }
            }

            // Create firewall policy object
            let policy: INetFwPolicy2 =
                CoCreateInstance(&NetFwPolicy2, None, CLSCTX_ALL).map_err(|e| {
                    NetworkError::generic(format!("Failed to create firewall policy: {}", e))
                })?;

            let rules = policy.Rules().map_err(|e| {
                NetworkError::generic(format!("Failed to get firewall rules: {}", e))
            })?;

            let rule_name = self.get_rule_name(port, protocol);
            let rule_name_bstr = BSTR::from(rule_name.as_str());

            rules
                .Remove(&rule_name_bstr)
                .map_err(|e| NetworkError::generic(format!("Failed to remove rule: {}", e)))?;

            info!(
                "Removed Windows Firewall exception for {} port {}",
                protocol.as_str(),
                port
            );
            Ok(())
        }
    }

    fn has_port_exception(&self, port: u16, protocol: Protocol) -> NetworkResult<bool> {
        use windows::core::BSTR;
        use windows::Win32::NetworkManagement::WindowsFirewall::{INetFwPolicy2, NetFwPolicy2};
        use windows::Win32::System::Com::{
            CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_APARTMENTTHREADED,
        };

        unsafe {
            // Initialize COM
            if let Err(e) = CoInitializeEx(None, COINIT_APARTMENTTHREADED) {
                if e.code().0 != 0x00000001 {
                    return Err(NetworkError::generic(format!(
                        "Failed to initialize COM: {}",
                        e
                    )));
                }
            }

            // Create firewall policy object
            let policy: INetFwPolicy2 =
                CoCreateInstance(&NetFwPolicy2, None, CLSCTX_ALL).map_err(|e| {
                    NetworkError::generic(format!("Failed to create firewall policy: {}", e))
                })?;

            let rules = policy.Rules().map_err(|e| {
                NetworkError::generic(format!("Failed to get firewall rules: {}", e))
            })?;

            let rule_name = self.get_rule_name(port, protocol);
            let rule_name_bstr = BSTR::from(rule_name.as_str());

            match rules.Item(&rule_name_bstr) {
                Ok(_) => Ok(true),
                Err(_) => Ok(false),
            }
        }
    }
}

/// Linux firewall helper (UFW/iptables).
#[cfg(target_os = "linux")]
pub struct LinuxFirewallHelper {
    app_name: String,
}

#[cfg(target_os = "linux")]
impl LinuxFirewallHelper {
    pub fn new(app_name: &str) -> NetworkResult<Self> {
        Ok(Self {
            app_name: app_name.to_string(),
        })
    }
}

#[cfg(target_os = "linux")]
impl FirewallIntegration for LinuxFirewallHelper {
    fn add_port_exception(&self, port: u16, protocol: Protocol) -> NetworkResult<()> {
        use std::process::Command;

        // Try UFW first
        let output = Command::new("ufw")
            .args(&[
                "allow",
                &format!("{}/{}", port, protocol.as_str().to_lowercase()),
            ])
            .output();

        match output {
            Ok(result) if result.status.success() => {
                info!(
                    "Added UFW firewall exception for {} port {}",
                    protocol.as_str(),
                    port
                );
                Ok(())
            }
            _ => {
                // UFW failed or not available, try iptables
                let output = Command::new("iptables")
                    .args(&[
                        "-A",
                        "INPUT",
                        "-p",
                        protocol.as_str().to_lowercase().as_str(),
                        "--dport",
                        &port.to_string(),
                        "-j",
                        "ACCEPT",
                    ])
                    .output();

                match output {
                    Ok(result) if result.status.success() => {
                        info!(
                            "Added iptables firewall exception for {} port {}",
                            protocol.as_str(),
                            port
                        );
                        Ok(())
                    }
                    _ => {
                        warn!("Failed to add Linux firewall exception - may require manual configuration");
                        Ok(()) // Don't fail, just warn
                    }
                }
            }
        }
    }

    fn remove_port_exception(&self, port: u16, protocol: Protocol) -> NetworkResult<()> {
        use std::process::Command;

        // Try UFW first
        let output = Command::new("ufw")
            .args(&[
                "delete",
                "allow",
                &format!("{}/{}", port, protocol.as_str().to_lowercase()),
            ])
            .output();

        match output {
            Ok(result) if result.status.success() => {
                info!(
                    "Removed UFW firewall exception for {} port {}",
                    protocol.as_str(),
                    port
                );
                Ok(())
            }
            _ => {
                // Try iptables
                let output = Command::new("iptables")
                    .args(&[
                        "-D",
                        "INPUT",
                        "-p",
                        protocol.as_str().to_lowercase().as_str(),
                        "--dport",
                        &port.to_string(),
                        "-j",
                        "ACCEPT",
                    ])
                    .output();

                match output {
                    Ok(result) if result.status.success() => {
                        info!(
                            "Removed iptables firewall exception for {} port {}",
                            protocol.as_str(),
                            port
                        );
                        Ok(())
                    }
                    _ => {
                        warn!("Failed to remove Linux firewall exception");
                        Ok(())
                    }
                }
            }
        }
    }

    fn has_port_exception(&self, _port: u16, _protocol: Protocol) -> NetworkResult<bool> {
        // Checking Linux firewall rules requires parsing complex output
        // For now, return Ok(false) and let add_port_exception handle it
        Ok(false)
    }
}

/// macOS firewall helper (pfctl).
#[cfg(target_os = "macos")]
pub struct MacFirewallHelper {
    #[allow(dead_code)]
    app_name: String,
}

#[cfg(target_os = "macos")]
impl MacFirewallHelper {
    pub fn new(app_name: &str) -> NetworkResult<Self> {
        Ok(Self {
            app_name: app_name.to_string(),
        })
    }
}

#[cfg(target_os = "macos")]
impl FirewallIntegration for MacFirewallHelper {
    fn add_port_exception(&self, _port: u16, _protocol: Protocol) -> NetworkResult<()> {
        // macOS application-level firewall doesn't require port rules
        // The OS will prompt the user when the application tries to listen
        debug!("macOS firewall does not require explicit port rules");
        Ok(())
    }

    fn remove_port_exception(&self, _port: u16, _protocol: Protocol) -> NetworkResult<()> {
        debug!("macOS firewall does not require explicit port rules");
        Ok(())
    }

    fn has_port_exception(&self, _port: u16, _protocol: Protocol) -> NetworkResult<bool> {
        Ok(true) // macOS handles this at application level
    }
}

/// Create platform-specific firewall helper.
pub fn create_firewall_helper(app_name: &str) -> NetworkResult<Box<dyn FirewallIntegration>> {
    #[cfg(target_os = "windows")]
    {
        Ok(Box::new(WindowsFirewallHelper::new(app_name)?))
    }

    #[cfg(target_os = "linux")]
    {
        Ok(Box::new(LinuxFirewallHelper::new(app_name)?))
    }

    #[cfg(target_os = "macos")]
    {
        Ok(Box::new(MacFirewallHelper::new(app_name)?))
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        Err(NetworkError::generic(
            "Unsupported platform for firewall integration",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_conversion() {
        assert_eq!(Protocol::Tcp.as_str(), "TCP");
        assert_eq!(Protocol::Udp.as_str(), "UDP");

        #[cfg(target_os = "windows")]
        {
            assert_eq!(Protocol::Tcp.as_net_fw_protocol(), 6);
            assert_eq!(Protocol::Udp.as_net_fw_protocol(), 17);
        }
    }

    #[test]
    fn create_helper() {
        // This should not panic on supported platforms
        let result = create_firewall_helper("Test App");
        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
        assert!(result.is_ok());
    }
}
