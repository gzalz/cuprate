use std::collections::HashSet;
use std::time::Duration;

use futures::channel::mpsc::SendError;
use futures::stream::FuturesUnordered;
use futures::{channel::mpsc, SinkExt, Stream, StreamExt, TryFutureExt, TryStream};
use monero_serai::rpc::HttpRpc;
use tokio::time::timeout;
use tower::discover::Change;
use tower::ServiceExt;
use tracing::instrument;

use super::Rpc;
use crate::Database;

#[instrument]
async fn check_rpc(addr: String) -> Option<Rpc<HttpRpc>> {
    tracing::debug!("Sending request to node.");
    let rpc = HttpRpc::new(addr.clone()).ok()?;
    // make sure the RPC is actually reachable
    timeout(Duration::from_secs(2), rpc.get_height())
        .await
        .ok()?
        .ok()?;

    tracing::debug!("Node sent ok response.");

    Some(Rpc::new_http(addr))
}

struct RPCDiscover<T> {
    rpc: T,
    initial_list: Vec<String>,
    ok_channel: mpsc::Sender<Change<usize, Rpc<HttpRpc>>>,
    already_connected: HashSet<String>,
}

impl<T: Database> RPCDiscover<T> {
    async fn found_rpc(&mut self, rpc: Rpc<HttpRpc>) -> Result<(), SendError> {
        if self.already_connected.contains(&rpc.addr) {
            return Ok(());
        }

        tracing::info!("Found node to connect to: {}", &rpc.addr);

        let addr = rpc.addr.clone();
        self.ok_channel
            .send(Change::Insert(self.already_connected.len(), rpc))
            .await?;
        self.already_connected.insert(addr);

        Ok(())
    }

    pub async fn run(mut self) {
        loop {
            if !self.initial_list.is_empty() {
                let mut fut =
                    FuturesUnordered::from_iter(self.initial_list.drain(..).map(check_rpc));

                while let Some(res) = fut.next().await {
                    if let Some(rpc) = res {
                        if self.found_rpc(rpc).await.is_err() {
                            tracing::info!("Stopping RPC discover channel closed!");
                            return;
                        }
                    }
                }
            }

            if self.already_connected.len() > 100 {
                tracing::info!("Stopping RPC discover, connected to 100 nodes!");
            }

            tokio::time::sleep(Duration::from_secs(2)).await

            // TODO: RPC request to get more peers
        }
    }
}