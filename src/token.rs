use alloy::primitives::Address;
use alloy::providers::Provider;
use alloy_sol_types::sol;
use eyre::Result;
use thiserror::Error;
use tracing::error;

#[derive(Debug, Error)]
pub enum TokenInfoError {
    #[error("Failed to fetch token decimal")]
    TokenDecimalFetchFailed,

    #[error("Failed to fetch token symbol")]
    TokenSymbolFetchFailed,
}

sol! {
    #[sol(rpc)]
    interface IERC20 {
        function decimals() external view returns (uint8);
        function symbol() external view returns (string);
    }
}

//call token contract, return token's decimal and symbol
pub async fn load_token_info(token: Address, provider: impl Provider) -> Result<(u8, String)> {
    if token == Address::ZERO {
        return Ok((18, "ETH".to_string()));
    }

    let contract = IERC20::new(token, &provider);

    let decimals = match contract.decimals().call().await {
        Ok(dec) => dec,
        Err(e) => {
            error!("Failed to fetch token decimal {}: {}", token, e);
            return Err(TokenInfoError::TokenDecimalFetchFailed.into());
        }
    };
    let symbol = match contract.symbol().call().await {
        Ok(sym) => sym,
        Err(e) => {
            error!("Failed to fetch token symbol {}: {}", token, e);
            return Err(TokenInfoError::TokenSymbolFetchFailed.into());
        }
    };

    Ok((decimals, symbol))
}
