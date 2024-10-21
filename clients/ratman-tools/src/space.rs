use crate::{base_args::BaseArgs, encode_map, OutputFormat};
use clap::ArgMatches;
use libratman::{
    api::{RatmanIpc, RatmanNamespaceExt},
    tokio::{
        fs::File,
        io::{AsyncReadExt, AsyncWriteExt},
    },
    types::Address,
    Result,
};
use std::{collections::BTreeMap, sync::Arc, time::Duration};

pub async fn register(
    ipc: &Arc<RatmanIpc>,
    base_args: BaseArgs,
    matches: &ArgMatches,
) -> Result<()> {
    let (_, auth) = base_args.identity_data?;
    let space_file = matches.get_one::<String>("file_name").unwrap();
    let (pubkey, privkey) = libratman::generate_space_key();

    ipc.namespace_register(auth, pubkey, privkey).await?;

    let mut f = File::open(space_file).await?;
    f.write_all(
        format!(
            "{}",
            encode_map(
                vec![
                    ("pubkey", pubkey.to_string()),
                    ("privkey", privkey.to_string())
                ],
                base_args.out_fmt
            )
        )
        .as_bytes(),
    )
    .await?;

    Ok(())
}

pub async fn up(ipc: &Arc<RatmanIpc>, base_args: BaseArgs, matches: &ArgMatches) -> Result<()> {
    let (addr, auth) = base_args.identity_data?;
    let space_file = matches.get_one::<String>("file_name").unwrap();

    let mut f = File::open(space_file).await?;
    let mut buf = String::new();
    f.read_to_string(&mut buf).await?;

    let (pubkey, _) = match base_args.out_fmt {
        OutputFormat::Lines => {
            let mut lines = buf.lines();

            let pubkey = lines.next().unwrap().split("=").last().unwrap().to_string();
            let privkey = lines.next().unwrap().split("=").last().unwrap().to_string();

            (
                Address::from_string(&pubkey),
                Address::from_string(&privkey),
            )
        }
        OutputFormat::Json => {
            let mut map: BTreeMap<String, String> = serde_json::from_str(buf.as_str()).unwrap();
            (
                Address::from_string(&map.remove("pubkey").unwrap()),
                Address::from_string(&map.remove("privkey").unwrap()),
            )
        }
    };

    ipc.namespace_up(addr, auth, pubkey).await?;

    Ok(())
}

pub async fn down(ipc: &Arc<RatmanIpc>, base_args: BaseArgs, matches: &ArgMatches) -> Result<()> {
    let (addr, auth) = base_args.identity_data?;
    let space_file = matches.get_one::<String>("file_name").unwrap();

    let mut f = File::open(space_file).await?;
    let mut buf = String::new();
    f.read_to_string(&mut buf).await?;

    let (pubkey, _) = match base_args.out_fmt {
        OutputFormat::Lines => {
            let mut lines = buf.lines();

            let pubkey = lines.next().unwrap().split("=").last().unwrap().to_string();
            let privkey = lines.next().unwrap().split("=").last().unwrap().to_string();

            (
                Address::from_string(&pubkey),
                Address::from_string(&privkey),
            )
        }
        OutputFormat::Json => {
            let mut map: BTreeMap<String, String> = serde_json::from_str(buf.as_str()).unwrap();
            (
                Address::from_string(&map.remove("pubkey").unwrap()),
                Address::from_string(&map.remove("privkey").unwrap()),
            )
        }
    };

    ipc.namespace_down(addr, auth, pubkey).await?;
    Ok(())
}

pub async fn anycast(
    ipc: &Arc<RatmanIpc>,
    base_args: BaseArgs,
    matches: &ArgMatches,
) -> Result<()> {
    let (addr, auth) = base_args.identity_data?;
    let space_file = matches.get_one::<String>("file_name").unwrap();
    let timeout = matches.get_one::<u64>("timeout").unwrap();

    let mut f = File::open(space_file).await?;
    let mut buf = String::new();
    f.read_to_string(&mut buf).await?;

    let (pubkey, _) = match base_args.out_fmt {
        OutputFormat::Lines => {
            let mut lines = buf.lines();

            let pubkey = lines.next().unwrap().split("=").last().unwrap().to_string();
            let privkey = lines.next().unwrap().split("=").last().unwrap().to_string();

            (
                Address::from_string(&pubkey),
                Address::from_string(&privkey),
            )
        }
        OutputFormat::Json => {
            let mut map: BTreeMap<String, String> = serde_json::from_str(buf.as_str()).unwrap();
            (
                Address::from_string(&map.remove("pubkey").unwrap()),
                Address::from_string(&map.remove("privkey").unwrap()),
            )
        }
    };

    let addrs = ipc
        .namespace_anycast_probe(addr, auth, pubkey, Duration::from_millis(*timeout))
        .await?;

    println!(
        "{}",
        encode_map(
            addrs
                .into_iter()
                .map(|(addr, duration)| (addr, format!("{}ms", duration.as_millis()))),
            base_args.out_fmt
        )
    );

    Ok(())
}
