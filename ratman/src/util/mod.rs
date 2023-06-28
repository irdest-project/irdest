mod config;
pub use config::Config;

mod platform;
pub use platform::Os;

use async_std::channel::{Receiver, Sender};
use tracing_subscriber::{filter::LevelFilter, fmt, EnvFilter};

/// A convevient Sender/Receiver pair for a type
pub(crate) type IoPair<T> = (Sender<T>, Receiver<T>);

/// Print a log message and exit
// TODO: turn into macro
pub fn elog<S: Into<String>>(msg: S, code: u16) -> ! {
    error!("{}", msg.into());
    std::process::exit(code.into());
}

/// Get XDG_DATA_HOME from the environment
pub(crate) fn env_xdg_data() -> Option<String> {
    std::env::var("XDG_DATA_HOME").ok()
}

pub fn setup_logging(lvl: &str, syslog: bool) {
    let filter = EnvFilter::default()
        .add_directive(match lvl {
            "trace" => LevelFilter::TRACE.into(),
            "debug" => LevelFilter::DEBUG.into(),
            "info" => LevelFilter::INFO.into(),
            "warn" => LevelFilter::WARN.into(),
            "error" => LevelFilter::ERROR.into(),
            _ => unreachable!(),
        })
        .add_directive("async_io=error".parse().unwrap())
        .add_directive("async_std=error".parse().unwrap())
        .add_directive("mio=error".parse().unwrap())
        .add_directive("polling=error".parse().unwrap())
        .add_directive("tide=warn".parse().unwrap())
        .add_directive("trust_dns_proto=error".parse().unwrap())
        .add_directive("trust_dns_resolver=warn".parse().unwrap());

    // Initialise the logger
    if syslog {
        let identity = std::ffi::CStr::from_bytes_with_nul(b"ratmand\0").unwrap();
        let facility = Default::default();
        let syslog =
            tracing_syslog::Syslog::new(identity, tracing_syslog::Options::LOG_PID, facility);
        fmt()
            .with_ansi(false)
            .with_env_filter(filter)
            .with_writer(syslog)
            .init();
    } else {
        #[cfg(not(feature = "android"))]
        fmt().with_env_filter(filter).init();
    }

    info!("Initialised logger: welcome to ratmand!");
}