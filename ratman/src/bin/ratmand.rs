// SPDX-FileCopyrightText: 2022-2023 Katharina Fey <kookie@spacekookie.de>
// SPDX-FileCopyrightText: 2022 Lux <lux@lux.name>
//
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-AppStore

//! Ratman daemon entrypoint
use libratman::{rt::AsyncSystem, types::Os};
use ratmand::{
    config::ConfigTree,
    start_with_configuration,
    util::{cli, codes, fork::sysv_daemonize_app},
};
use std::{env, path::PathBuf};

// async fn generate_default_config(_path: &PathBuf) {
//     let _cfg = ConfigTree::default_in_memory();
// }

//#[async_std::main]
fn main() {
    let arg_matches = cli::build_cli();

    let cfg_path = arg_matches
        .value_of("CONFIG")
        .map(|s| PathBuf::new().join(s))
        .unwrap_or_else(|| Os::xdg_config_path().join("ratmand.kdl"));

    let state_path = arg_matches
        .value_of("STATE_DIR")
        .map(|s| PathBuf::new().join(s))
        .unwrap_or_else(|| Os::xdg_data_path());

    let sys_startup = AsyncSystem::new("ratmand-startup".to_owned(), 2);

    // Check if we were tasked to generate the default
    // configuration, then execute this and exit afterwards.
    if let Some(generate_matches) = arg_matches.subcommand_matches("generate") {
        let mut cfg = ConfigTree::default_in_memory();
        if let Some(patches) = generate_matches.values_of("CONFIG_PATCH") {
            for patch in patches {
                let (key, value) = patch.split_once('=').unwrap_or_else(|| ("", ""));

                if key == "" || value == "" {
                    eprintln!("Invalid patch syntax!  Usage: <key>=<value>");
                }

                // turn things into actual types
                if value == "true" {
                    cfg = cfg.patch(key, true);
                } else if value == "false" {
                    cfg = cfg.patch(key, false);
                } else if let Some(num) = value.parse::<i64>().ok() {
                    cfg = cfg.patch(key, num);
                } else {
                    cfg = cfg.patch(key, value);
                }
            }
        }

        if let Some(peers) = generate_matches.values_of("ADD_PEER") {
            for peer in peers {
                cfg = cfg.patch_list("ratmand/peers", peer);
            }
        }

        if let Err(e) = sys_startup.exec(async { cfg.write_changes(&cfg_path).await }) {
            eprintln!("failed to write default configuration: {}", e);
        }

        std::process::exit(0);
    }

    // Since this code runs before the logger initialisation we're
    // limited to eprintln and exiting the application manually if
    // something goes catastrophically wrong.

    eprintln!("Loading config from {:?}", cfg_path);

    let mut config = match sys_startup.exec(ConfigTree::load_path(&cfg_path)) {
        Ok(cfg) => cfg,
        Err(_) => {
            // If the configuration couldn't be loaded we assume that
            // it just doesn't exist yet and we try to create it.
            let cfg = ConfigTree::default_in_memory();
            if let Err(_) = sys_startup.exec(cfg.write_changes(&cfg_path)) {
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

    // Override the ephemeral value
    // if arg_matches.is_present("EPHEMERAL") {
    //     config = config.patch("ratmand/ephemeral", true);
    // }

    // Override the config verbosity value with the CLI value if desired
    if let Some(verbosity) = arg_matches.value_of("VERBOSE") {
        if let Ok(rust_log_env) = env::var("RUST_LOG") {
            let full_verbose = format!("{rust_log_env},{verbosity}");
            config = config.patch("ratmand/verbosity", full_verbose.as_str());
        } else {
            config = config.patch("ratmand/verbosity", verbosity);
        }
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
        if let Err(err) = sysv_daemonize_app(config, state_path) {
            eprintln!("ratmand suffered fatal error: {}", err);
            std::process::exit(codes::FATAL as i32);
        }
    }
    // Otherwise just normally initialise the Context
    else {
        start_with_configuration(config, state_path)
    }
}
