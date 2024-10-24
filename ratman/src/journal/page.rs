// SPDX-FileCopyrightText: 2024 Katharina Fey <kookie@spacekookie.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-AppStore

use crate::{journal::types::BlockData, storage::block::StorageBlock};
use async_eris::{Block, BlockReference, BlockStorage};
use async_trait::async_trait;
use fjall::PartitionHandle;
use libratman::{
    frame::{FrameGenerator, FrameParser},
    tokio::task::spawn_blocking,
    EncodingError, RatmanError, Result,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{io::Result as IoResult, marker::PhantomData};

/// Represent a single logical page in the fjall database
///
/// A cache page has an associated Rust type which it will serialize and
/// deserialize for storage. Each page can be configured with a custom block
/// size if that is desired.
pub struct CachePage<T: Serialize + DeserializeOwned>(pub PartitionHandle, pub PhantomData<T>);

impl<T: Serialize + DeserializeOwned> CachePage<T> {
    pub async fn insert(&self, key: String, value: &T) -> Result<()> {
        let bin = bincode::serialize(value)?;
        let handle = self.0.clone();
        spawn_blocking(move || handle.insert(key, bin)).await??;
        Ok(())
    }

    pub async fn remove(&self, key: String) -> Result<()> {
        let handle = self.0.clone();
        spawn_blocking(move || handle.remove(key)).await??;
        Ok(())
    }

    pub async fn get(&self, key: &String) -> Result<Option<T>> {
        let handle = self.0.clone();
        let key = key.clone();

        let bin_data = spawn_blocking(move || handle.get(key)).await??;
        Ok(bin_data
            .map(|bin_data| bincode::deserialize(&*bin_data).expect("failed to decode data")))
    }

    /// Perform a prefix key search and filter out invalid entries
    pub fn prefix<'key>(
        &'key self,
        prefix: &'key String,
    ) -> impl DoubleEndedIterator<Item = (String, T)> + 'key {
        self.0
            .prefix(prefix)
            // filter out read read failures
            .filter(|x| x.is_ok())
            .map(|x| x.unwrap())
            // then deserialise the data
            .map(|(item_key, item_data)| {
                (
                    String::from_utf8(item_key.to_vec()),
                    bincode::deserialize(&*item_data),
                )
            })
            // then filter out encoding failures
            .filter(|(x, y)| x.is_ok() && y.is_ok())
            .map(|(x, y)| (x.unwrap(), y.unwrap()))
    }

    /// Get an iterator over all valid entries on this page and their keys
    pub fn iter(&self) -> Vec<(String, T)> {
        self.0
            .iter()
            .filter_map(|item| match item {
                Ok((key, val)) => Some((
                    String::from_utf8(key.to_vec()).unwrap(),
                    bincode::deserialize(&*val).unwrap(),
                )),
                Err(_) => None,
            })
            .collect()
    }

    /// Iterate over this cache page while pre-filtering desired values
    pub fn iter_filter(&self, f: impl FnMut(&(String, T)) -> bool) -> Vec<(String, T)> {
        self.0
            .iter()
            .filter_map(|item| match item {
                Ok((key, val)) => Some((
                    String::from_utf8(key.to_vec()).unwrap(),
                    bincode::deserialize(&*val).unwrap(),
                )),
                Err(_) => None,
            })
            .filter(f)
            .collect()
    }

    /// Count the number of entries in this cache page
    pub fn len(&self) -> Result<usize> {
        Ok(self.0.len()?)
    }
}

#[async_trait]
impl<const L: usize> BlockStorage<L> for CachePage<BlockData> {
    async fn store(&self, block: &Block<L>) -> IoResult<()> {
        self.insert(
            block.reference().to_string(),
            &BlockData {
                // We can unwrap here because the blocks were previously
                // verified to be valid
                data: StorageBlock::reconstruct(block.as_slice()).unwrap(),
                valid: true,
            },
        )
        .await?;
        Ok(())
    }

    async fn fetch(&self, reference: &BlockReference) -> IoResult<Option<Block<L>>> {
        match self.get(&reference.to_string()).await {
            Err(RatmanError::Io(io)) => Err(io),
            Ok(Some(BlockData { data, valid })) if valid => Ok(Some(data.to_block())),
            _ => Ok(None),
        }
    }
}

/// Allow serialising of non-serde types
///
/// Because serde doesn't allow serialising arrays larger than 32 items and many
/// frames in Irdest use 64 byte arrays for metadata side channels we need to
/// wrap these types in our own serde type.
///
/// A SerdeFrameType can be constructed via `From<FrameGenerator>` and
/// deserialised via `to_frametype()` which invokes `FrameParser` (both from
/// libratman).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SerdeFrameType<T>(Vec<u8>, PhantomData<T>);

impl<T: FrameGenerator> From<T> for SerdeFrameType<T> {
    fn from(input: T) -> SerdeFrameType<T> {
        let mut frame_buf = vec![];
        input.generate(&mut frame_buf).unwrap();
        SerdeFrameType(frame_buf, PhantomData)
    }
}

impl<T: FrameParser<Output = Result<T>>> SerdeFrameType<T> {
    pub fn maybe_inner(&self) -> Result<T> {
        match T::parse(&self.0) {
            Ok((_, t)) => t,
            Err(e) => Err(RatmanError::Encoding(EncodingError::from(e))),
        }
    }
}
