use alloy::primitives::U256;
use alloy::primitives::utils::format_units;
use num_bigint::BigInt;
use num_rational::Ratio;
use num_traits::One;
use thiserror::Error;
use tracing::info;

#[derive(Debug, Error)]
pub enum FormatError {
    #[error("Failed to format price")]
    FormatPriceFailed,
}

pub fn calculate_prices(
    sqrt_price_x96_str: String,
    decimal_token0: u32,
    decimal_token1: u32,
    token0_symbol: &String,
    token1_symbol: &String,
) -> Result<(BigInt, BigInt), FormatError> {
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

    //convert type to U256 for format the price
    let buy_one_token0_u256 = U256::from_be_slice(&buy_one_token0.to_signed_bytes_be());
    let buy_one_token1_u256 = U256::from_be_slice(&buy_one_token1.to_signed_bytes_be());

    // format BigInt prices into human-readable strings
    let formatted_token0_price = match format_units(buy_one_token0_u256, "ether") {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("Failed to format token0 price: {}", e);
            return Err(FormatError::FormatPriceFailed.into());
        }
    };

    let formatted_token1_price = match format_units(buy_one_token1_u256, "ether") {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("Failed to format token0 price: {}", e);
            return Err(FormatError::FormatPriceFailed.into());
        }
    };

    // logs token prices for both directions:
    // 1 token0 = *price* token1
    // 1 token1 = *price* token0
    info!(
        "1 {:?} =  {:?} {:?}, 1 {:?} = {:?} {:?}",
        token0_symbol,
        formatted_token0_price,
        token1_symbol,
        token1_symbol,
        formatted_token1_price,
        token0_symbol
    );

    Ok((buy_one_token0, buy_one_token1))
}
