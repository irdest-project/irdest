// SPDX-FileCopyrightText: 2022 Yureka Lilian <yuka@yuka.dev>
//
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-AppStore

use async_eris as eris;
use eris::{BlockSize, MemoryStorage};
use std::collections::HashMap;

#[tokio::main]
async fn main() {
    let examples = vec![b"Hello world!".as_slice()];

    for content in examples {
        let key = [0; 32];
        let blocks = MemoryStorage::new(HashMap::new());
        let read_capability = eris::encode(&mut &*content, &key, BlockSize::_1K, &blocks)
            .await
            .unwrap();
        println!("{}", read_capability.urn());
        println!("{:?}", read_capability);
        for (reference, block) in &*blocks.read().unwrap() {
            println!(
                "{}: {}",
                base32::encode(base32::Alphabet::RFC4648 { padding: false }, &**reference),
                base32::encode(base32::Alphabet::RFC4648 { padding: false }, &block)
            );
        }

        let mut decoded = vec![];
        eris::decode(&mut decoded, &read_capability, &blocks)
            .await
            .unwrap();
        println!("{}", String::from_utf8_lossy(&decoded));
    }
}
