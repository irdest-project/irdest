use kdl::{KdlDocument, KdlEntry, KdlNode};

const DEFAULT_CONFIG: &'static str = r#"// This is the ratmand router and peering driver config.
// Other Irdest applications are configured elsewhere!

// Main ratmand router/server configuration section.
// Peering driver (netmod) configurations are contained in their own sections below
settings "ratmand" {
    // ratmand can be set up to log at different levels.
    // Valid options: trace, debug, info, warn, error.  'debug' is usually a happy medium.
    verbosity "debug"

    // Enable (or disable) the web configuration dashboard
    enable_dashboard true
    dashboard_bind "localhost:9000"

    // Configure whether ratmand should remain attached to the calling process, or turn itself into a daemon.
    // It's recommended not to change this value unless you know you need it
    daemonize false
    pid_file "/tmp/ratmand.pid"

    // Configure what socket ratmand uses for local client connections.
    // If you change this setting, you will also have to tell every connecting client the new address.
    api_bind "localhost:9020"

    // By default ratmand will reject peering requests from other routers that haven't explicitly been configured (see below).
    // If you're running a publicly accessible ratmand instance, it's recommended you change this to 'true'
    accept_unknown_peers false

    // This section of the configuration is used to tell ratmand about some initial peers.
    // The format is as follows: <driver>:<hostname/ip address>:[<port>].  The port may be omitted,
    // if the default (9000) is used.  As an example, we have included two public test servers,
    // that are run by Irdest developers.
    peers {
        // inet:hyperion.kookie.space:9000
        // inet:hub.irde.st:9000
    }

    // Alternatively you can include a list of peers in an external file
    // peer_file "~/.config/ratmand/peers.txt"
}

// Configuration for the 'inet' internet overlay driver, used to peer between ratmand instances via an existing internet connection
settings "inet" {
    // Enable (or disable) the inet driver at runtime
    enable true

    // Configure what socket netmod_inet should use to accept incoming peering connections.
    // Note that changing the bind port will mean others need to manually specify it in their peering config
    bind "0.0.0.0:9000"
}

// Configuration for the 'lan' local peer discovery driver.
// It uses IPv6 multicast messages to find other peers on the same network
// (if the host network supports it -- many corporate or event networks filter these messages for spam reasons)
settings "lan" {
    // Enable (or disable) the lan driver at runtime
    enable true

    // You may change the port used for automatic discovery, but it is not recommended that you do,
    // since it will mean that most other ratmand instances won't be able to see you.
    // If however that is exactly what you want, then you may change the value here
    discovery_port 9001

    // If your computer has multple interfaces that can be used for local discovery, you can explicitly select one of them here.
    // Currently discovery is only supported via a single interface (this will change in the future!).
    // discovery_iface "eth0"
}

// Configuration for the 'lora' wireless radio driver.  See the user manual on how to setup the wireless radio
settings "lora" {
    // Enable (or disable) the lora driver at runtime
    enable false

    // Configure where the lora driver can find the serial connection to the lora modem.
    // Since lora is a very low-bandwidth radio channel, only 9600 baud connections are currently supported.
    serial_port "/dev/ttyUSB0"
    serial_baud 9600

}

// Anything unclear?  Check out the user manual: https://docs.irde.st/user/
// Still unclear?  Feel free to ask us questions on Matrix or on our community mailing list :)
"#;

/// Create a new default configuration from scratch
pub fn create_new_default() -> KdlDocument {
    DEFAULT_CONFIG.parse().expect("error in built-in configuration (if you are not a developer of Irdest, please report this problem!): ")
}

/// Take an existing KDL configuration and update it to the latest format
pub fn update_existing(kdl_doc: KdlDocument) -> KdlDocument {
    kdl_doc
}
