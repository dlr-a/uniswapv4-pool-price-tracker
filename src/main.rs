mod calc;
mod pool;
mod token;

use crate::pool::listen_pool;
use alloy::primitives::address;
use alloy::providers::{ProviderBuilder, WsConnect};
use alloy_primitives::FixedBytes;
use eyre::Result;
use std::env;
use tracing::error;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let pool_ids_str = env::var("POOL_IDS")?;

    let rpc_url = "wss://ethereum-rpc.publicnode.com";
    let ws = WsConnect::new(rpc_url);
    let provider = ProviderBuilder::new().connect_ws(ws).await?;

    let pool_manager = address!("0x000000000004444c5dc75cB358380D2e3dE08A90");
    let position_manager = address!("0xbd216513d74c8cf14cf4747e6aaa6420ff64ee9e");

    let pool_ids: Vec<FixedBytes<32>> = pool_ids_str
        .split(',')
        .filter_map(|s| s.trim().parse::<FixedBytes<32>>().ok())
        .collect();

    let mut handles = Vec::new();

    for pool_id in pool_ids {
        let provider = provider.clone();

        handles.push(tokio::spawn(async move {
            if let Err(e) = listen_pool(pool_id, pool_manager, position_manager, provider).await {
                error!("listener for pool {:?} stopped: {}", pool_id, e);
            }
        }));
    }

    for handle in handles {
        let _ = handle.await;
    }

    Ok(())
}
