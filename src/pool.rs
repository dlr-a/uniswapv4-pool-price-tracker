use crate::{calc::calculate_prices, token::load_token_info};
use alloy::primitives::Address;
use alloy::{
    providers::Provider,
    rpc::types::{BlockNumberOrTag, Filter},
};
use alloy_primitives::FixedBytes;
use alloy_sol_types::SolEvent;
use alloy_sol_types::sol;
use eyre::Result;
use futures_util::stream::StreamExt;
use thiserror::Error;
use tracing::error;
use tracing::info;

#[derive(Debug, Error)]
pub enum TokenError {
    #[error("Failed to fetch token info from address")]
    TokenInfoFetchFailed,
}

#[derive(Debug, Error)]
pub enum LogError {
    #[error("Failed to subscribe logs")]
    LogSubscriptionFailed,

    #[error("Failed to fetch sqrt price")]
    SqrtPriceFetchFailed,
}

#[derive(Error, Debug)]
pub enum PriceError {
    #[error("Failed to calculate price for pool {0}, tokens {1}/{2}: {3}")]
    CalculationFailed(FixedBytes<32>, String, String, String),
}

sol! {
    #[sol(rpc)]
    interface PositionManager {
        function poolKeys(bytes25) returns (
            address currency0,
            address currency1,
            uint24 fee,
            int24 tickSpacing,
            address hooks
        );
    }

    type PoolId is bytes32;

    event Swap(
        PoolId indexed id,
        address indexed sender,
        int128 amount0,
        int128 amount1,
        uint160 sqrtPriceX96,
        uint128 liquidity,
        int24 tick,
        uint24 fee
    );
}

pub async fn listen_pool(
    pool_id: FixedBytes<32>,
    pool_manager: Address,
    position_manager: Address,
    provider: impl Provider + 'static,
) -> Result<()> {
    let pool_id_bytes25 = FixedBytes::<25>::from_slice(&pool_id.as_slice()[..25]);
    let manager = PositionManager::new(position_manager, &provider);
    let key = manager.poolKeys(pool_id_bytes25).call().await?;
    // key.currency0, key.currency1

    let token0 = key.currency0;
    let token1 = key.currency1;

    //call token contracts with load_token_info function for fetch decimals and symbols
    let (dec0, sym0) = match load_token_info(token0, &provider).await {
        Ok(info) => info,
        Err(e) => {
            error!("Failed to load token info for token {:?}: {}", token0, e);
            return Err(TokenError::TokenInfoFetchFailed.into());
        }
    };
    let (dec1, sym1) = match load_token_info(token1, &provider).await {
        Ok(info) => info,
        Err(e) => {
            error!("Failed to load token info for token {:?}: {}", token1, e);
            return Err(TokenError::TokenInfoFetchFailed.into());
        }
    };

    //filter to listen only for swap events from this pool
    let filter = Filter::new()
        .address(pool_manager)
        .event_signature(Swap::SIGNATURE_HASH) // elle string yazma, sol!'dan al
        .topic1(pool_id) // sadece bu havuzun swap'leri
        .from_block(BlockNumberOrTag::Latest);

    let sub = match provider.subscribe_logs(&filter).await {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to subscribe logs with filter {:?}: {}", filter, e);
            return Err(LogError::LogSubscriptionFailed.into());
        }
    };

    let mut stream = sub.into_stream();

    info!("Listening pool: {:?}", pool_id);

    while let Some(log) = stream.next().await {
        let Swap { sqrtPriceX96, .. } = match log.log_decode() {
            Ok(decoded) => decoded.inner.data,
            Err(e) => {
                tracing::error!("Failed to decode log: {}", e);
                return Err(LogError::SqrtPriceFetchFailed.into());
            }
        };

        //calculate price with sqrtpricex96 and token decimals
        let price = match calculate_prices(
            sqrtPriceX96.to_string(),
            dec0 as u32,
            dec1 as u32,
            &sym0,
            &sym1,
        ) {
            Ok(p) => p,
            Err(e) => {
                tracing::error!("Failed to calculate price for {}/{}: {}", sym0, sym1, e);
                return Err(PriceError::CalculationFailed(
                    pool_id,
                    sym0.clone(),
                    sym1.clone(),
                    e.to_string(),
                )
                .into());
            }
        };

        info!("SQRT_PRICE: {:#?} from pool: {:?}", price, pool_id);
    }

    Ok(())
}
