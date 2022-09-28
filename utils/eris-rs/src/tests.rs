// SPDX-FileCopyrightText: 2022 Katharina Fey <kookie@spacekookie.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-AppStore

use crate::{
    Block, BlockKey, BlockReference, BlockSize, BlockStorage, MemoryStorage, ReadCapability,
};
use serde::{Deserialize, Serialize};

use std::collections::BTreeMap;
use std::{fs::File, io::Read, path::Path};

#[derive(Serialize, Deserialize, Debug)]
pub struct ReadCapabilityTest {
    #[serde(rename = "block-size")]
    pub block_size: usize,
    pub level: u8,
    #[serde(rename = "root-reference")]
    pub root_reference: String,
    #[serde(rename = "root-key")]
    pub root_key: String,
}

impl TryFrom<&ReadCapabilityTest> for ReadCapability {
    type Error = crate::Error;

    fn try_from(cap_test: &ReadCapabilityTest) -> Result<Self, Self::Error> {
        Ok(ReadCapability {
            root_reference: BlockReference::try_from(&cap_test.root_reference)?,
            root_key: BlockKey::try_from(&cap_test.root_key)?,
            level: cap_test.level,
            block_size: cap_test.block_size,
        })
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TestVectorContent {
    pub id: u32,
    #[serde(rename = "spec-version")]
    pub spec_version: String,
    pub name: String,
    pub description: String,
    pub content: String,
    #[serde(rename = "convergence-secret")]
    pub convergence_secret: String,
    #[serde(rename = "block-size")]
    pub block_size: usize,
    #[serde(rename = "read-capability")]
    pub read_capability: ReadCapabilityTest,
    pub urn: String,
    pub blocks: BTreeMap<String, String>,
}

impl TestVectorContent {
    pub async fn blocks_to_blocks(&self) -> Result<MemoryStorage, crate::Error> {
        let mut store = MemoryStorage::new();

        for (block_id, block) in self.blocks.iter() {
            let block_ref = BlockReference::try_from(block_id)?;
            match self.read_capability.block_size {
                1024 => {
                    let block = Block::<1024>::try_from(block)?;
                    assert_eq!(block_ref, block.reference());
                    store.store(&block).await.unwrap();
                }
                32768 => {
                    let block = Block::<32768>::try_from(block)?;
                    assert_eq!(block_ref, block.reference());
                    store.store(&block).await.unwrap();
                }
                _ => panic!("Unsupported block size!"),
            };
        }

        Ok(store)
    }
}

#[derive(Debug)]
pub struct TestHarness {
    blocks: MemoryStorage,
    read_cap: ReadCapability,
    _test: TestVectorContent,
}

impl TestHarness {
    async fn load(path: &Path) -> Option<Box<Self>> {
        let mut buf = String::new();
        let mut f = File::open(path).unwrap();
        f.read_to_string(&mut buf).unwrap();

        let vector_content: TestVectorContent = match serde_json::from_str(buf.as_str()) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("Error: {:?}.", e);
                return None;
            }
        };

        let read_cap = ReadCapability::try_from(&vector_content.read_capability).ok()?;
        let blocks = vector_content.blocks_to_blocks().await.ok()?;

        Some(Box::new(Self {
            blocks,
            read_cap,
            _test: vector_content,
        }))
    }
}

async fn verify_input_content(harness: &TestHarness) -> bool {
    let input_content = crate::vardecode_base32(&harness._test.content).unwrap();

    let secret: [u8; 32] = crate::decode_base32(harness._test.convergence_secret.as_str()).unwrap();
    let block_size = match harness._test.block_size {
        1024 => BlockSize::_1K,
        32768 => BlockSize::_32K,
        _ => unreachable!(),
    };
    let mut new_store = MemoryStorage::new();

    let new_read_cap = crate::encode(
        &mut input_content.as_slice(),
        &secret,
        block_size,
        &mut new_store,
    )
    .await
    .unwrap();

    harness.blocks == new_store && harness.read_cap.root_reference == new_read_cap.root_reference
}

async fn run_test_for_vector(path: &Path, tx: async_std::channel::Sender<()>) {
    let harness = match TestHarness::load(path).await {
        Some(h) => Box::new(h),
        _ => {
            eprintln!(
                "An error occured loading {:?} and the test will now fail!",
                path
            );
            std::process::exit(2);
        }
    };

    println!(
        "Loading file: {:?} has resulted in {} blocks",
        path,
        harness.blocks.len()
    );

    // Decode input content and verify that this results in the same
    // set of blocks as in the test harness file!
    assert!(verify_input_content(&harness).await);

    // If we reach this point this vector was successfully parsed,
    // decoded, and re-encoded.
    tx.send(()).await.unwrap();
}

#[async_std::test]
async fn run_vectors() {
    let mut test_vectors = std::fs::read_dir("./res/eris-test-vectors")
        .unwrap()
        .filter_map(|res| match res {
            Ok(entry) if entry.file_name().to_str().unwrap().ends_with(".json") => Some(entry),
            _ => None, // Not important?
        })
        .collect::<Vec<_>>();
    test_vectors
        .as_mut_slice()
        .sort_by_key(|entry| entry.file_name().to_str().unwrap().to_owned());

    for res in test_vectors {
        // We do this little dance here because otherwise it's very
        // easy to get stack overflow errors in this test scenario!
        let path = res.path();
        let (tx, rx) = async_std::channel::bounded(1);
        async_std::task::spawn(async move {
            run_test_for_vector(path.as_path(), tx).await;
        });

        rx.recv().await.unwrap();
    }
}
