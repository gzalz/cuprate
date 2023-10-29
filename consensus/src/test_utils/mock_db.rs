use futures::FutureExt;
use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, RwLock},
    task::{Context, Poll},
};

use cuprate_common::BlockID;
use tower::{BoxError, Service};

use crate::{DatabaseRequest, DatabaseResponse, ExtendedBlockHeader, HardFork};

#[derive(Default, Debug, Clone, Copy)]
pub struct DummyBlockExtendedHeader {
    pub version: Option<HardFork>,
    pub vote: Option<HardFork>,

    pub timestamp: Option<u64>,
    pub cumulative_difficulty: Option<u128>,

    pub block_weight: Option<usize>,
    pub long_term_weight: Option<usize>,
}

impl From<DummyBlockExtendedHeader> for ExtendedBlockHeader {
    fn from(value: DummyBlockExtendedHeader) -> Self {
        ExtendedBlockHeader {
            version: value.version.unwrap_or(HardFork::V1),
            vote: value.vote.unwrap_or(HardFork::V1),
            timestamp: value.timestamp.unwrap_or_default(),
            cumulative_difficulty: value.cumulative_difficulty.unwrap_or_default(),
            block_weight: value.block_weight.unwrap_or_default(),
            long_term_weight: value.long_term_weight.unwrap_or_default(),
        }
    }
}

impl DummyBlockExtendedHeader {
    pub fn with_weight_into(
        mut self,
        weight: usize,
        long_term_weight: usize,
    ) -> DummyBlockExtendedHeader {
        self.block_weight = Some(weight);
        self.long_term_weight = Some(long_term_weight);
        self
    }

    pub fn with_hard_fork_info(
        mut self,
        version: HardFork,
        vote: HardFork,
    ) -> DummyBlockExtendedHeader {
        self.vote = Some(vote);
        self.version = Some(version);
        self
    }

    pub fn with_difficulty_info(
        mut self,
        timestamp: u64,
        cumulative_difficulty: u128,
    ) -> DummyBlockExtendedHeader {
        self.timestamp = Some(timestamp);
        self.cumulative_difficulty = Some(cumulative_difficulty);
        self
    }
}

#[derive(Debug, Default)]
pub struct DummyDatabaseBuilder {
    blocks: Vec<DummyBlockExtendedHeader>,
}

impl DummyDatabaseBuilder {
    pub fn add_block(&mut self, block: DummyBlockExtendedHeader) {
        self.blocks.push(block);
    }

    pub fn finish(self) -> DummyDatabase {
        DummyDatabase {
            blocks: Arc::new(self.blocks.into()),
        }
    }
}

#[derive(Clone)]
pub struct DummyDatabase {
    blocks: Arc<RwLock<Vec<DummyBlockExtendedHeader>>>,
}

impl Service<DatabaseRequest> for DummyDatabase {
    type Response = DatabaseResponse;
    type Error = BoxError;
    type Future =
        Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: DatabaseRequest) -> Self::Future {
        let blocks = self.blocks.clone();

        async move {
            Ok(match req {
                DatabaseRequest::BlockExtendedHeader(BlockID::Height(id)) => {
                    DatabaseResponse::BlockExtendedHeader(
                        blocks
                            .read()
                            .unwrap()
                            .get(usize::try_from(id).unwrap())
                            .copied()
                            .map(Into::into)
                            .ok_or("block not in database!")?,
                    )
                }
                DatabaseRequest::BlockHash(id) => {
                    let mut hash = [0; 32];
                    hash[0..8].copy_from_slice(&id.to_le_bytes());
                    DatabaseResponse::BlockHash(hash)
                }
                DatabaseRequest::BlockExtendedHeaderInRange(range) => {
                    DatabaseResponse::BlockExtendedHeaderInRange(
                        blocks
                            .read()
                            .unwrap()
                            .iter()
                            .take(usize::try_from(range.end).unwrap())
                            .skip(usize::try_from(range.start).unwrap())
                            .copied()
                            .map(Into::into)
                            .collect(),
                    )
                }
                DatabaseRequest::ChainHeight => {
                    let height = u64::try_from(blocks.read().unwrap().len()).unwrap();

                    let mut top_hash = [0; 32];
                    top_hash[0..8].copy_from_slice(&height.to_le_bytes());

                    DatabaseResponse::ChainHeight(height, top_hash)
                }
                DatabaseRequest::GeneratedCoins => DatabaseResponse::GeneratedCoins(0),
                _ => unimplemented!("the context svc should not need these requests!"),
            })
        }
        .boxed()
    }
}