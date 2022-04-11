use anyhow::Result;
use ethers_core::types::{BlockNumber, Chain};
use ethers_etherscan::{
    account::{
        ERC20TokenTransferEvent, ERC721TokenTransferEvent, NormalTransaction, TokenQueryOption,
    },
    Client,
};
use serde::{Deserialize, Serialize};

#[cxx::bridge(namespace = "com::crypto::game_sdk")]
mod ffi {
    /// Raw transaction details (extracted from Cronoscan/Etherscan or BlockScout API)
    #[derive(Debug, PartialEq, Eq)]
    pub struct RawTxDetail {
        /// Transaction hash
        pub hash: String,
        /// the hexadecimal address of the receiver
        pub to_address: String,
        /// the hexadecimal address of the sender
        pub from_address: String,
        /// the value sent in decimal (in base tokens)
        pub value: String,
        /// block number when it happened
        pub block_no: u64,
        /// the time it happened
        pub timestamp: String,
        /// the address of the contract (if no contract, it's an empty string)
        pub contract_address: String,
    }

    /// Token ownership result detail from BlockScout API
    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    pub struct RawTokenResult {
        /// how many tokens are owned by the address
        pub balance: String,
        /// the deployed contract address
        #[serde(rename = "contractAddress")]
        pub contract_address: String,
        /// the number of decimal places
        pub decimals: String,
        /// the human-readable name of the token
        pub name: String,
        /// the ticker for the token
        pub symbol: String,
        /// the token type (ERC-20, ERC-721, ERC-1155)
        #[serde(rename = "type")]
        pub token_type: String,
    }

    pub enum QueryOption {
        ByContract,
        ByAddressAndContract,
        ByAddress,
    }

    extern "Rust" {
        pub fn get_transaction_history_blocking(
            address: String,
            api_key: String,
        ) -> Result<Vec<RawTxDetail>>;
        pub fn get_erc20_transfer_history_blocking(
            address: String,
            contract_address: String,
            option: QueryOption,
            api_key: String,
        ) -> Result<Vec<RawTxDetail>>;
        pub fn get_erc721_transfer_blocking(
            address: String,
            contract_address: String,
            option: QueryOption,
            api_key: String,
        ) -> Result<Vec<RawTxDetail>>;
        pub fn get_tokens_blocking(
            blockscout_base_url: String,
            account_address: String,
        ) -> Result<Vec<RawTokenResult>>;
        pub fn get_token_transfers_blocking(
            blockscout_base_url: String,
            address: String,
            contract_address: String,
            option: QueryOption,
        ) -> Result<Vec<RawTxDetail>>;

    }
}

use ffi::{QueryOption, RawTokenResult, RawTxDetail};

/// returns the transactions of a given address.
/// The API key can be obtained from https://cronoscan.com
pub fn get_transaction_history_blocking(
    address: String,
    api_key: String,
) -> Result<Vec<RawTxDetail>> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async move { get_transaction_history(&address, api_key).await })
}

/// returns the ERC20 transfers of a given address of a given contract.
/// (address can be empty if option is ByContract)
/// default option is by address
/// The API key can be obtained from https://cronoscan.com
pub fn get_erc20_transfer_history_blocking(
    address: String,
    contract_address: String,
    option: QueryOption,
    api_key: String,
) -> Result<Vec<RawTxDetail>> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async move {
        get_erc20_transfer_history(&address, &contract_address, option, api_key).await
    })
}

/// returns the ERC721 transfers of a given address of a given contract.
/// (address can be empty if option is ByContract)
/// default option is by address
/// The API key can be obtained from https://cronoscan.com
pub fn get_erc721_transfer_blocking(
    address: String,
    contract_address: String,
    option: QueryOption,
    api_key: String,
) -> Result<Vec<RawTxDetail>> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async move {
        get_erc721_transfer_history(&address, &contract_address, option, api_key).await
    })
}

/// given the BlockScout REST API base url and the account address (hexadecimal),
/// it will return the list of all owned tokens
/// (ref: https://cronos.org/explorer/testnet3/api-docs)
pub fn get_tokens_blocking(
    blockscout_base_url: String,
    account_address: String,
) -> Result<Vec<RawTokenResult>> {
    let blockscout_url =
        format!("{blockscout_base_url}?module=account&action=tokenlist&address={account_address}");
    let resp = reqwest::blocking::get(&blockscout_url)?.json::<RawResponse<RawTokenResult>>()?;
    Ok(resp.result)
}

/// given the BlockScout REST API base url and the account address (hexadecimal; required)
/// and optional contract address (hexadecimal; optional -- it can be empty if the option is ByAddress),
/// it will return all the token transfers (ERC20, ERC721... in the newer BlockScout
/// releases, also ERC1155)
/// (ref: https://cronos.org/explorer/testnet3/api-docs)
/// NOTE: QueryOption::ByContract is not supported by BlockScout
pub fn get_token_transfers_blocking(
    blockscout_base_url: String,
    address: String,
    contract_address: String,
    option: QueryOption,
) -> Result<Vec<RawTxDetail>> {
    let blockscout_url = match option {
        QueryOption::ByAddress => {
            format!("{blockscout_base_url}?module=account&action=tokentx&address={address}")
        }
        QueryOption::ByAddressAndContract => {
            format!(
                "{blockscout_base_url}?module=account&action=tokentx&address={address}&contractaddress={contract_address}"
            )
        }
        _ => {
            anyhow::bail!("unsupported option")
        }
    };
    let resp =
        reqwest::blocking::get(&blockscout_url)?.json::<RawResponse<RawBlockScoutTransfer>>()?;

    Ok(resp.result.iter().flat_map(TryInto::try_into).collect())
}

#[derive(Serialize, Deserialize)]
struct RawResponse<R> {
    message: String,
    result: Vec<R>,
    status: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawBlockScoutTransfer {
    block_hash: String,
    block_number: String,
    confirmations: String,
    contract_address: String,
    cumulative_gas_used: String,
    from: String,
    gas: String,
    gas_price: String,
    gas_used: String,
    hash: String,
    input: String,
    log_index: String,
    nonce: String,
    time_stamp: String,
    to: String,
    token_decimal: String,
    token_name: String,
    token_symbol: String,
    transaction_index: String,
    value: String,
}

impl TryFrom<&RawBlockScoutTransfer> for RawTxDetail {
    type Error = anyhow::Error;

    fn try_from(tx: &RawBlockScoutTransfer) -> Result<Self, Self::Error> {
        let block_no = tx.block_number.parse::<u64>()?;
        Ok(Self {
            hash: tx.hash.clone(),
            to_address: tx.to.clone(),
            from_address: tx.from.clone(),
            value: tx.value.clone(),
            block_no,
            timestamp: tx.time_stamp.clone(),
            contract_address: tx.contract_address.clone(),
        })
    }
}

impl From<&NormalTransaction> for RawTxDetail {
    fn from(tx: &NormalTransaction) -> Self {
        let block_no: u64 = match tx.block_number {
            BlockNumber::Number(block_no) => block_no.0[0],
            _ => 0,
        };
        RawTxDetail {
            hash: tx.hash.value().map(|x| x.to_string()).unwrap_or_default(),
            to_address: tx.to.map(|x| x.to_string()).unwrap_or_default(),
            from_address: tx.from.value().map(|x| x.to_string()).unwrap_or_default(),
            value: tx.value.to_string(),
            block_no,
            timestamp: tx.time_stamp.clone(),
            contract_address: tx
                .contract_address
                .value()
                .map(|x| x.to_string())
                .unwrap_or_default(),
        }
    }
}

impl From<&ERC20TokenTransferEvent> for RawTxDetail {
    fn from(tx: &ERC20TokenTransferEvent) -> Self {
        let block_no: u64 = match tx.block_number {
            BlockNumber::Number(block_no) => block_no.0[0],
            _ => 0,
        };
        RawTxDetail {
            hash: tx.hash.to_string(),
            to_address: tx.to.map(|x| x.to_string()).unwrap_or_default(),
            from_address: tx.from.to_string(),
            value: tx.value.to_string(),
            block_no,
            timestamp: tx.time_stamp.clone(),
            contract_address: tx.contract_address.to_string(),
        }
    }
}

impl From<&ERC721TokenTransferEvent> for RawTxDetail {
    fn from(tx: &ERC721TokenTransferEvent) -> Self {
        let block_no: u64 = match tx.block_number {
            BlockNumber::Number(block_no) => block_no.0[0],
            _ => 0,
        };
        RawTxDetail {
            hash: tx.hash.to_string(),
            to_address: tx.to.map(|x| x.to_string()).unwrap_or_default(),
            from_address: tx.from.to_string(),
            value: tx.token_id.to_string(),
            block_no,
            timestamp: tx.time_stamp.clone(),
            contract_address: tx.contract_address.to_string(),
        }
    }
}

async fn get_transaction_history(address: &str, api_key: String) -> Result<Vec<RawTxDetail>> {
    let client = Client::new(Chain::Cronos, api_key)?;
    let transactions = client.get_transactions(&address.parse()?, None).await?;
    Ok(transactions.iter().map(|tx| tx.into()).collect())
}

async fn get_erc20_transfer_history(
    address: &str,
    contract_address: &str,
    option: QueryOption,
    api_key: String,
) -> Result<Vec<RawTxDetail>> {
    let client = Client::new(Chain::Cronos, api_key)?;
    let token_query = match option {
        QueryOption::ByContract => TokenQueryOption::ByContract(contract_address.parse()?),
        QueryOption::ByAddressAndContract => {
            TokenQueryOption::ByAddressAndContract(address.parse()?, contract_address.parse()?)
        }
        _ => TokenQueryOption::ByAddress(address.parse()?),
    };
    let transactions = client
        .get_erc20_token_transfer_events(token_query, None)
        .await?;
    Ok(transactions.iter().map(|tx| tx.into()).collect())
}

async fn get_erc721_transfer_history(
    address: &str,
    contract_address: &str,
    option: QueryOption,
    api_key: String,
) -> Result<Vec<RawTxDetail>> {
    let client = Client::new(Chain::Cronos, api_key)?;
    let token_query = match option {
        QueryOption::ByContract => TokenQueryOption::ByContract(contract_address.parse()?),
        QueryOption::ByAddressAndContract => {
            TokenQueryOption::ByAddressAndContract(address.parse()?, contract_address.parse()?)
        }
        _ => TokenQueryOption::ByAddress(address.parse()?),
    };
    let transactions = client
        .get_erc721_token_transfer_events(token_query, None)
        .await?;
    Ok(transactions.iter().map(|tx| tx.into()).collect())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    pub fn test_get_tokens() {
        let expected: Vec<RawTokenResult> = serde_json::from_str(r#"[{"balance":"36330128084034373866325","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"2883410031878","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"161","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"1","contractAddress":"0x1e0edbea442cfeff05ed1b01f0c38ecb768de0e0","decimals":"","name":"NFT Gold","symbol":"Gold","type":"ERC-1155"},{"balance":"1","contractAddress":"0x93d0c9a35c43f6bc999416a06aadf21e68b29eba","decimals":"","name":"Unique One","symbol":"UNE","type":"ERC-1155"},{"balance":"1","contractAddress":"0x93d0c9a35c43f6bc999416a06aadf21e68b29eba","decimals":"","name":"Unique One","symbol":"UNE","type":"ERC-1155"},{"balance":"1","contractAddress":"0x57aaaf5a61b6a370f981b7826843694cfa4774e1","decimals":"","name":"Protector","symbol":"サイタマ","type":"ERC-1155"},{"balance":"1","contractAddress":"0x57aaaf5a61b6a370f981b7826843694cfa4774e1","decimals":"","name":"Protector","symbol":"サイタマ","type":"ERC-1155"},{"balance":"1","contractAddress":"0x57aaaf5a61b6a370f981b7826843694cfa4774e1","decimals":"","name":"Protector","symbol":"サイタマ","type":"ERC-1155"},{"balance":"4","contractAddress":"0x93d0c9a35c43f6bc999416a06aadf21e68b29eba","decimals":"","name":"Unique One","symbol":"UNE","type":"ERC-1155"},{"balance":"1","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"1","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"1","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"1","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"1","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"2","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"1","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"2","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"2","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"1","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"2","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"2","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"2","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"2","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"1","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"1","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"2","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"2","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"2","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"2","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"1","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"2","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"1","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"2","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"2","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"2","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"2","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"2","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"2","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"2","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"2","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"2","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"1","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"2","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"2","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"1","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"1","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"1","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"1","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"1","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"1","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"1","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"1","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"1","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"1","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"1","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"1","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"1","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"1","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"1","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"5","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"4","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"1","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"9","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"1","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"4","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"9","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"1","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"9","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"1","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"1","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"1","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"1","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"1","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"1","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"1","contractAddress":"0xed1efc6efceaab9f6d609fec89c9e675bf1efb0a","decimals":"","name":"SNAFU","symbol":"SNAFU","type":"ERC-1155"},{"balance":"36616853110389525899548","contractAddress":"0x27b9c2bd4baea18abdf49169054c1c1c12af9862","decimals":"18","name":"SNAFU","symbol":"SNAFU","type":"ERC-20"},{"balance":"73000000000000000000","contractAddress":"0x586f8a53c24d8d35a9f49e94d09058560791803e","decimals":"18","name":"NFTOPIUM","symbol":"NTP","type":"ERC-20"},{"balance":"763467280363239051","contractAddress":"0x6a023ccd1ff6f2045c3309768ead9e68f978f6e1","decimals":"18","name":"Wrapped Ether on xDai","symbol":"WETH","type":"ERC-20"},{"balance":"1","contractAddress":"0x90fda259cfbdb74f1804e921f523e660bfbe698d","decimals":"","name":"Unique Pixie","symbol":"UPIXIE","type":"ERC-721"},{"balance":"3000000000000000000","contractAddress":"0x9c58bacc331c9aa871afd802db6379a98e80cedb","decimals":"18","name":"Gnosis Token on xDai","symbol":"GNO","type":"ERC-20"}]"#).expect("parse");

        // blacksout somestimes works, sometimes not
        let max_count = 10;
        for _ in 0..max_count {
            let actual_result = get_tokens_blocking(
                "https://blockscout.com/xdai/mainnet/api".into(),
                "0x652d53227d7013f3FbBeA542443Dc2eeF05719De".into(),
            );
            match actual_result {
                Ok(actual) => {
                    assert_eq!(actual.len(), expected.len());
                    for (a, b) in actual.iter().zip(expected.iter()) {
                        // assert_eq!(a.balance, b.balance);
                        assert_eq!(a.contract_address, b.contract_address);
                        assert_eq!(a.decimals, b.decimals);
                        assert_eq!(a.name, b.name);
                        assert_eq!(a.symbol, b.symbol);
                        assert_eq!(a.token_type, b.token_type);
                    }
                    return;
                }
                Err(_e) => {
                    // try more
                    // wait for 1 second
                    std::thread::sleep(std::time::Duration::from_millis(1000));
                }
            }
        } // end for for loop
        panic!("test_get_tokens failed");
    }

    #[test]
    pub fn test_get_token_transactions() {
        let expected: Vec<RawBlockScoutTransfer> = serde_json::from_str(
            r#"[
            {
              "value": "200000000000000000000",
              "blockHash": "0x1456b7934898b7c735967e849effc4dc45e84ff32c6c2e130d572cb9589ca652",
              "blockNumber": "2088372",
              "confirmations": "537920",
              "contractAddress": "0x715b4d660148c477e03358f8b0315ed4088fe89a",
              "cumulativeGasUsed": "31760308",
              "from": "0x9ad08de843158b0a4f8efdae6ea49caf77bbf13f",
              "gas": "145930",
              "gasPrice": "2021527882398",
              "gasUsed": "97287",
              "hash": "0x0890c4dce61da8713db5844fa0ae0aa73b74ea6ccfa91c90065ae80471cd908c",
              "input": "0x3363315c000000000000000000000000841a15d12aec9c6039fd132c2fbff112ed355700",
              "logIndex": "104",
              "nonce": "13",
              "timeStamp": "1646318156",
              "to": "0x841a15d12aec9c6039fd132c2fbff112ed355700",
              "tokenDecimal": "18",
              "tokenName": "DAI",
              "tokenSymbol": "DAI",
              "transactionIndex": "37"
            },
            {
              "value": "200000000",
              "blockHash": "0x307edc85e8d5bde617857831427cb90e0c9b2b6c455d6f3a56918c3ee4aa009d",
              "blockNumber": "2039569",
              "confirmations": "586723",
              "contractAddress": "0xf0307093f23311fe6776a7742db619eb3df62969",
              "cumulativeGasUsed": "97352",
              "from": "0x9ad08de843158b0a4f8efdae6ea49caf77bbf13f",
              "gas": "146028",
              "gasPrice": "5017552095418",
              "gasUsed": "97352",
              "hash": "0x7939078155138ed15a10343627a4e6b8d623d63bdd69479bf62d7872dee587d6",
              "input": "0xd8215830000000000000000000000000841a15d12aec9c6039fd132c2fbff112ed355700",
              "logIndex": "0",
              "nonce": "18",
              "timeStamp": "1646036708",
              "to": "0x841a15d12aec9c6039fd132c2fbff112ed355700",
              "tokenDecimal": "6",
              "tokenName": "USDC",
              "tokenSymbol": "USDC",
              "transactionIndex": "0"
            }
          ]"#,
        )
        .expect("parse");
        let expected: Vec<RawTxDetail> = expected.iter().flat_map(TryInto::try_into).collect();
        let actual = get_token_transfers_blocking(
            "https://cronos.org/explorer/testnet3/api".to_string(),
            "0x841a15D12aEc9c6039FD132c2FbFF112eD355700".to_string(),
            "".to_string(),
            QueryOption::ByAddress,
        )
        .expect("blockscout query");
        assert_eq!(actual, expected);
    }
}
