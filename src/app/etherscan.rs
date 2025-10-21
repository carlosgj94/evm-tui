use crate::app::AddressRef;
use alloy::primitives::U256;
use serde::Deserialize;
use std::{fmt, str::FromStr, time::Duration};

const ETHERSCAN_V2_BASE: &str = "https://api.etherscan.io/v2/api";

#[derive(Debug, Clone)]
pub struct TransactionListSource {
    pub label: &'static str,
    pub api_version: &'static str,
}

#[derive(Debug, Clone)]
pub struct AddressTransaction {
    pub hash: String,
    pub block_number: u64,
    pub from: String,
    pub to: Option<String>,
    pub value_wei: U256,
    pub is_error: bool,
}

#[derive(Debug)]
pub enum TransactionFetchError {
    MissingApiKey,
    UnsupportedChain(String),
    Http(reqwest::Error),
    Parse(serde_json::Error),
    Api(String),
}

impl fmt::Display for TransactionFetchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransactionFetchError::MissingApiKey => f.write_str("no Etherscan API key configured"),
            TransactionFetchError::UnsupportedChain(chain) => {
                write!(f, "no Etherscan-compatible chain mapping for \"{chain}\"")
            }
            TransactionFetchError::Http(err) => write!(f, "network error: {err}"),
            TransactionFetchError::Parse(err) => write!(f, "response parse error: {err}"),
            TransactionFetchError::Api(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for TransactionFetchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            TransactionFetchError::Http(err) => Some(err),
            TransactionFetchError::Parse(err) => Some(err),
            _ => None,
        }
    }
}

impl From<reqwest::Error> for TransactionFetchError {
    fn from(value: reqwest::Error) -> Self {
        TransactionFetchError::Http(value)
    }
}

impl From<serde_json::Error> for TransactionFetchError {
    fn from(value: serde_json::Error) -> Self {
        TransactionFetchError::Parse(value)
    }
}

#[derive(Debug, Clone)]
struct ChainConfig {
    chain_id: u64,
    label: &'static str,
}

const ETHEREUM_MAINNET: ChainConfig = ChainConfig {
    chain_id: 1,
    label: "Etherscan",
};
const ARBITRUM_ONE: ChainConfig = ChainConfig {
    chain_id: 42161,
    label: "Arbiscan",
};
const BASE_MAINNET: ChainConfig = ChainConfig {
    chain_id: 8453,
    label: "Basescan",
};
const ETHEREUM_SEPOLIA: ChainConfig = ChainConfig {
    chain_id: 11155111,
    label: "Etherscan (Sepolia)",
};

fn resolve_chain(chain: &str) -> Option<&'static ChainConfig> {
    let normalized = chain.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "mainnet" | "ethereum" | "ethereum mainnet" => Some(&ETHEREUM_MAINNET),
        "arbitrum" | "arbitrum one" => Some(&ARBITRUM_ONE),
        "base" | "base mainnet" => Some(&BASE_MAINNET),
        "sepolia" | "ethereum sepolia" => Some(&ETHEREUM_SEPOLIA),
        _ => None,
    }
}

pub async fn fetch_address_transactions(
    address: &AddressRef,
    api_key: Option<&str>,
    limit: usize,
) -> Result<(Vec<AddressTransaction>, TransactionListSource), TransactionFetchError> {
    let api_key = api_key
        .filter(|value| !value.trim().is_empty())
        .ok_or(TransactionFetchError::MissingApiKey)?;
    let chain = resolve_chain(&address.chain)
        .ok_or_else(|| TransactionFetchError::UnsupportedChain(address.chain.clone()))?;

    let client = reqwest::Client::builder()
        .user_agent("evm-tui/0.1.0")
        .timeout(Duration::from_secs(10))
        .build()?;

    let response = client
        .get(ETHERSCAN_V2_BASE)
        .query(&[
            ("chainid", chain.chain_id.to_string()),
            ("module", "account".into()),
            ("action", "txlist".into()),
            ("address", address.address.clone()),
            ("startblock", "0".into()),
            ("endblock", "999999999".into()),
            ("page", "1".into()),
            ("offset", limit.max(1).to_string()),
            ("sort", "desc".into()),
            ("apikey", api_key.to_string()),
        ])
        .send()
        .await?
        .error_for_status()?;

    let payload: ApiResponse = response.json().await?;

    let transactions = match payload.status.as_str() {
        "1" => serde_json::from_value::<Vec<RawTransaction>>(payload.result)?,
        "0" => {
            if payload
                .message
                .eq_ignore_ascii_case("No transactions found")
            {
                Vec::new()
            } else if let serde_json::Value::String(reason) = payload.result {
                return Err(TransactionFetchError::Api(reason));
            } else if let serde_json::Value::Array(_) = payload.result {
                serde_json::from_value::<Vec<RawTransaction>>(payload.result)?
            } else {
                return Err(TransactionFetchError::Api(payload.message));
            }
        }
        _ => {
            if let serde_json::Value::String(reason) = payload.result {
                return Err(TransactionFetchError::Api(reason));
            }
            return Err(TransactionFetchError::Api(payload.message));
        }
    };

    let parsed = transactions
        .into_iter()
        .map(|raw| {
            let block_number = raw.block_number.parse::<u64>().unwrap_or_default();
            let to = if raw.to.trim().is_empty() {
                None
            } else {
                Some(raw.to)
            };
            let value_wei = U256::from_str(&raw.value).unwrap_or_default();
            let is_error = matches!(raw.is_error.as_deref(), Some("1"))
                || matches!(raw.txreceipt_status.as_deref(), Some("0"));
            AddressTransaction {
                hash: raw.hash,
                block_number,
                from: raw.from,
                to,
                value_wei,
                is_error,
            }
        })
        .collect();

    Ok((
        parsed,
        TransactionListSource {
            label: chain.label,
            api_version: "v2",
        },
    ))
}

#[derive(Debug, Deserialize)]
struct ApiResponse {
    status: String,
    message: String,
    result: serde_json::Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawTransaction {
    block_number: String,
    hash: String,
    from: String,
    #[serde(default)]
    to: String,
    value: String,
    #[serde(default)]
    is_error: Option<String>,
    #[serde(default)]
    txreceipt_status: Option<String>,
}
