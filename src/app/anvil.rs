use alloy::{
    eips::{BlockId, BlockNumberOrTag},
    primitives::{Address, U256},
    providers::{Provider, ProviderBuilder},
};
use color_eyre::{Result, eyre::WrapErr};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountOverview {
    pub latest_block: u64,
    pub balance_wei: U256,
    pub transaction_count: u64,
    pub is_contract: bool,
}

fn normalize_url(rpc_url: &str) -> String {
    if rpc_url == "MOCK" {
        std::env::var("EVM_TUI_TEST_RPC").unwrap_or_else(|_| rpc_url.to_string())
    } else {
        rpc_url.to_string()
    }
}

async fn connect_provider(rpc_url: &str) -> Result<impl Provider> {
    ProviderBuilder::new()
        .connect(rpc_url)
        .await
        .wrap_err_with(|| format!("failed to connect to RPC provider at {rpc_url}"))
}

pub async fn fetch_account_overview(rpc_url: &str, target: Address) -> Result<AccountOverview> {
    let url = normalize_url(rpc_url);
    let provider = connect_provider(&url).await?;

    let latest_block = provider
        .get_block_number()
        .await
        .wrap_err("failed to query latest block number")?;

    let balance_wei = provider
        .get_balance(target)
        .block_id(BlockId::Number(BlockNumberOrTag::Latest))
        .await
        .wrap_err("failed to query balance")?;

    let transaction_count = provider
        .get_transaction_count(target)
        .block_id(BlockId::Number(BlockNumberOrTag::Latest))
        .await
        .wrap_err("failed to query transaction count")?;

    let code = provider
        .get_code_at(target)
        .block_id(BlockId::Number(BlockNumberOrTag::Latest))
        .await
        .wrap_err("failed to query account code")?;

    Ok(AccountOverview {
        latest_block,
        balance_wei,
        transaction_count,
        is_contract: !code.is_empty(),
    })
}

pub async fn fetch_latest_block(rpc_url: &str) -> Result<u64> {
    let url = normalize_url(rpc_url);
    let provider = connect_provider(&url).await?;
    provider
        .get_block_number()
        .await
        .wrap_err("failed to query latest block number")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{AddressRef, App, SecretsState};
    use alloy::primitives::{Address, U256};
    use std::str::FromStr;

    #[test]
    fn build_address_view_formats_overview() {
        let address = Address::from_str("0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266").unwrap();
        let addr_ref = AddressRef {
            label: "Test Label".into(),
            address: format!("{:#x}", address),
            chain: "Local".into(),
        };
        let overview = AccountOverview {
            latest_block: 42,
            balance_wei: U256::from(1_000_000_000_000_000_000u128),
            transaction_count: 7,
            is_contract: false,
        };

        let hydrated = crate::app::build_address_view(
            addr_ref,
            Some(overview.clone()),
            None,
            Some("https://eth.llamarpc.com".into()),
            None,
        );

        assert_eq!(
            hydrated.info.first().unwrap(),
            "RPC endpoint: https://eth.llamarpc.com"
        );
        assert!(
            hydrated
                .info
                .iter()
                .any(|line| line.contains("Latest block: 42"))
        );
        assert!(hydrated.info.iter().any(|line| line.contains("Balance:")));
        assert_eq!(hydrated.overview.as_ref(), Some(&overview));
    }

    #[tokio::test]
    async fn hydrate_address_without_rpc_returns_note() {
        let address = Address::from_str("0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266").unwrap();
        let addr_ref = AddressRef {
            label: "No RPC".into(),
            address: format!("{:#x}", address),
            chain: "Local".into(),
        };
        let secrets = SecretsState {
            etherscan_api_key: None,
            anvil_rpc_url: None,
        };

        let hydrated = App::hydrate_address(addr_ref, secrets).await;

        assert!(
            hydrated
                .info
                .first()
                .expect("info entry")
                .contains("Configure an Anvil RPC endpoint")
        );
        assert!(hydrated.overview.is_none());
    }
}
