// SPDX-FileCopyrightText: 2022-2023 Katharina Fey <kookie@spacekookie.de>
// SPDX-FileCopyrightText: 2022 Lux <lux@lux.name>
//
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-AppStore

//! Ratman daemon entrypoint

use ratmand::{
    config::ConfigTree,
    start_with_configuration,
    util::{cli, codes, fork::sysv_daemonize_app, Os},
};
use std::path::PathBuf;

async fn generate_default_config(path: &PathBuf) {
    let cfg = ConfigTree::default_in_memory();
    if let Err(e) = cfg.write_changes(path).await {
        eprintln!("failed to write default configuration: {}", e);
    }
}

#[async_std::main]
async fn main() {
    let arg_matches = cli::build_cli();

    let cfg_path = arg_matches
        .value_of("CONFIG")
        .map(|s| PathBuf::new().join(s))
        .unwrap_or_else(|| Os::xdg_config_path().join("ratmand.kdl"));

    // A bit hacky: check if we were tasked to generate the default
    // configuration, then execute this and exit afterwards.
    if let Some(_) = arg_matches.subcommand_matches("generate") {
        generate_default_config(&cfg_path).await;
        std::process::exit(1);
    }

    // Since this code runs before the logger initialisation we're
    // limited to eprintln and exiting the application manually if
    // something goes catastrophically wrong.

    let mut config = match ConfigTree::load_path(&cfg_path).await {
        Ok(cfg) => cfg,
        Err(_) => {
            // If the configuration couldn't be loaded we assume that
            // it just doesn't exist yet and we try to create it.
            let cfg = ConfigTree::default_in_memory();
            if let Err(_) = cfg.write_changes(&cfg_path).await {
                eprintln!(
                    "failed to write configuration to path {}",
                    cfg_path
                        .as_os_str()
                        .to_str()
                        .unwrap_or("<unprintable path>")
                );
            }
            cfg
        }
    };

    // config.pretty_print();

    // Override the config verbosity value with the CLI value if desired
    if let Some(verbosity) = arg_matches.value_of("VERBOSITY") {
        config = config.patch("ratmand/verbosity", verbosity);
    }

    let ratmand_tree = match config.get_subtree("ratmand") {
        Some(t) => t,
        None => {
            eprintln!("settings tree 'ratmand' is missing from the provided configuration!");
            std::process::exit(codes::INVALID_CONFIG as i32);
        }
    };

    // If the config says that ratmand should daemonize itself...
    if ratmand_tree.get_bool_value("daemonize").unwrap_or(false) {
        if let Err(err) = sysv_daemonize_app(config) {
            eprintln!("ratmand suffered fatal error: {}", err);
            std::process::exit(codes::FATAL as i32);
        }
    }
    // Otherwise just normally initialise the Context
    else {
        start_with_configuration(config).await
    }
}
