use alloy::primitives::address;
use alloy::providers::{Provider, ProviderBuilder, WsConnect};
use alloy::rpc::types::{BlockNumberOrTag, Filter};
use alloy_primitives::FixedBytes;
use alloy_sol_types::sol;
use eyre::Result;
use futures_util::stream::StreamExt;
use num_bigint::BigInt;
use num_rational::Ratio;
use num_traits::One;
use num_traits::Zero;
use std::env;
use uniswap_sdk_core::prelude::*;

fn format_price_readable(value: &BigInt, scale: u32, symbol: &str) -> String {
    let scale_factor = BigInt::from(10u64.pow(scale));
    let int_part = value / &scale_factor;
    let frac_part = value % &scale_factor;

    let mut int_str = int_part.to_string();

    let mut with_commas = String::new();
    let chars: Vec<char> = int_str.chars().collect();
    for (i, c) in chars.iter().rev().enumerate() {
        if i != 0 && i % 3 == 0 {
            with_commas.push(',');
        }
        with_commas.push(*c);
    }
    int_str = with_commas.chars().rev().collect();

    let mut frac_str = frac_part.to_string();
    let missing_zeros = scale as usize - frac_str.len();
    if missing_zeros > 0 {
        frac_str = "0".repeat(missing_zeros) + &frac_str;
    }

    let frac_trimmed = &frac_str;

    if int_part.is_zero() {
        return format!("0.{} {}", frac_trimmed.trim_end_matches('0'), symbol);
    }

    if frac_trimmed.is_empty() {
        format!("{} {}", int_str, symbol)
    } else {
        format!(
            "{}.{} {}",
            int_str,
            frac_trimmed.trim_end_matches('0'),
            symbol
        )
    }
}

fn calculate_prices(
    sqrt_price_x96_str: String,
    decimal_token0: u32,
    decimal_token1: u32,
    token0_symbol: &String,
    token1_symbol: &String,
) -> (BigInt, BigInt) {
    let sqrt_price_x96 = BigInt::parse_bytes(sqrt_price_x96_str.as_bytes(), 10).unwrap();
    let two_pow_96: BigInt = BigInt::one() << 96;

    // (sqrtPriceX96 / 2^96)^2
    let price_ratio = Ratio::new(sqrt_price_x96.clone(), two_pow_96.clone()).pow(2);

    // decimal factor = 10^(dec1 - dec0)
    let decimal_factor = Ratio::new(
        BigInt::from(10u64.pow(decimal_token1)),
        BigInt::from(10u64.pow(decimal_token0)),
    );

    let buy_one_token0_ratio: Ratio<BigInt> = price_ratio / decimal_factor;
    let buy_one_token1_ratio: Ratio<BigInt> = Ratio::one() / &buy_one_token0_ratio;

    // scale = 10^18
    let scale = BigInt::from(10u64.pow(18));

    let buy_one_token0 = (buy_one_token0_ratio.clone() * &scale).to_integer();
    let buy_one_token1 = (buy_one_token1_ratio.clone() * &scale).to_integer();

    println!(
        "Price token0→token1: {}",
        format_price_readable(&buy_one_token0, 18, token1_symbol)
    );
    println!(
        "Price token1→token0: {}",
        format_price_readable(&buy_one_token1, 18, token0_symbol)
    );

    (buy_one_token0, buy_one_token1)
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

    #[sol(rpc)]
    interface IUniswapV3Pool {
        function token0() external view returns (address);
        function token1() external view returns (address);
    }

    #[sol(rpc)]
    interface IERC20 {
        function decimals() external view returns (uint8);
        function symbol() external view returns (string);
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

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let pool_ids_str = env::var("POOL_IDS")?;

    let rpc_url = "wss://ethereum-rpc.publicnode.com";
    let ws = WsConnect::new(rpc_url);
    let provider = ProviderBuilder::new().connect_ws(ws).await?;

    let pool_addr = address!("000000000004444c5dc75cB358380D2e3dE08A90");
    let position_manager = address!("0xbd216513d74c8cf14cf4747e6aaa6420ff64ee9e");
    let pool_ids: Vec<FixedBytes<32>> = pool_ids_str
        .split(',')
        .filter_map(|s| s.trim().parse::<FixedBytes<32>>().ok())
        .collect();

    for pool_id in pool_ids {
        let provider = provider.clone();

        let pool_id_bytes25: FixedBytes<25> =
            FixedBytes::<25>::from_slice(&pool_id.as_slice()[..25]);

        tokio::spawn(async move {
            let filtered = Filter::new()
                .address(pool_addr)
                .event("Swap(bytes32,address,int128,int128,uint160,uint128,int24,uint24)")
                .from_block(BlockNumberOrTag::Latest)
                .topic1(pool_id);

            let sub = match provider.subscribe_logs(&filtered).await {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Failed to subscribe logs for pool {:?}: {}", pool_id, e);
                    return;
                }
            };
            let mut stream = sub.into_stream();

            let maganer = PositionManager::new(position_manager, &provider);

            let result = match maganer.poolKeys(pool_id_bytes25).call().await {
                Ok(res) => res,
                Err(e) => {
                    eprintln!(
                        "Failed to fetch poolKeys for pool_id {:?}: {}",
                        pool_id_bytes25, e
                    );
                    return;
                }
            };

            let (dec0, sym0) =
                if result.currency0 == address!("0x0000000000000000000000000000000000000000") {
                    (18u8, "ETH".to_string())
                } else {
                    let token0_contract = IERC20::new(result.currency0, &provider);
                    let dec = match token0_contract.decimals().call().await {
                        Ok(d) => d,
                        Err(e) => {
                            eprintln!(
                                "Failed to fetch decimals for token0 {:?}: {}",
                                result.currency0, e
                            );
                            18
                        }
                    };
                    let sym = match token0_contract.symbol().call().await {
                        Ok(s) => s,
                        Err(_) => "UNKNOWN".to_string(),
                    };
                    (dec, sym)
                };

            let (dec1, sym1) =
                if result.currency1 == address!("0x0000000000000000000000000000000000000000") {
                    (18u8, "ETH".to_string())
                } else {
                    let token1_contract = IERC20::new(result.currency1, &provider);
                    let dec = match token1_contract.decimals().call().await {
                        Ok(d) => d,
                        Err(e) => {
                            eprintln!(
                                "Failed to fetch decimals for token1 {:?}: {}",
                                result.currency1, e
                            );
                            18
                        }
                    };
                    let sym = match token1_contract.symbol().call().await {
                        Ok(s) => s,
                        Err(_) => "UNKNOWN".to_string(),
                    };
                    (dec, sym)
                };

            println!("Listening pool id: {:?}", pool_id);

            while let Some(log) = stream.next().await {
                let Swap {
                    id: _,
                    sender: _,
                    amount0: _,
                    amount1: _,
                    sqrtPriceX96,
                    liquidity: _,
                    tick: _,
                    fee: _,
                } = log.log_decode().unwrap().inner.data;

                let price = calculate_prices(
                    sqrtPriceX96.to_string(),
                    dec0 as u32,
                    dec1 as u32,
                    &sym0,
                    &sym1,
                );

                println!("SQRT_PRICE:, {:#?} from pool: {:?}", price, pool_id);
            }
        });
    }

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
    }
}
