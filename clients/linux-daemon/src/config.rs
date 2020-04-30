//! Configuration for the Linux daemon\
use serde::{Serialize, Deserialize};

trait NameableNetmod {
    fn name(&self) -> &str;
    fn netmod(&self) -> &str;
}

/// Top level configuration type for the daemon
#[derive(Serialize, Deserialize, Debug)]
struct Config {
    modules: Vec<NetmodConfig>
}

/// Network module configuration enum, for selecting which modules are to be loaded.
#[derive(Serialize, Deserialize, Debug)]
enum NetmodConfig {
    Udp(UdpNetmodConfig),
    Overlay(OverlayNetmodConfig)
    // TODO: Configs for more modules here
}

/// Network module config struct for netmod-udp. Prototype for other network modules.
#[derive(Serialize, Deserialize, Debug)]
struct UdpNetmodConfig {
    /// A semantic name, to make understanding error messages, etc, easier.
    name: String,
    /// The UDP address (IP or hostname) on which to listen
    // TODO: This actually isn't possible with the current netmod implementation
    address: Option<String>,
    /// The UDP port on which to listen
    port: u32
}

impl NameableNetmod for UdpNetmodConfig {
    fn name(&self) -> &str { &self.name }
    fn netmod(&self) -> &str { "netmod-udp" }
}

#[derive(Serialize, Deserialize, Debug)]
struct OverlayNetmodConfig {
    /// A semantic name, to make understanding error messages, etc, easier.
    name: String,
    /// The address of the server to connect to
    server: String,
    /// The port on the server to which to connect
    server_port: u32,
    /// The address to bind to; optional, will use system defaults
    bind_address: Option<String>,
    /// The port to bind to; optional, will use a random port as assigned by the system
    bind_port: Option<u32>
}

impl NameableNetmod for OverlayNetmodConfig {
    fn name(&self) -> &str { &self.name }
    fn netmod(&self) -> &str { "netmod-overlay" }
}