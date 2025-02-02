use crate::ffi::{WalletConnectCallback, WalletConnectTxCommon};
use anyhow::{anyhow, Result};
use defi_wallet_connect::session::SessionInfo;
use defi_wallet_connect::{Client, Metadata, WCMiddleware};
use defi_wallet_connect::{ClientChannelMessage, ClientChannelMessageType};

use ethers::core::types::transaction::eip2718::TypedTransaction;
use url::Url;

use crate::ffi::WalletConnectSessionInfo;
use cxx::UniquePtr;
use ethers::prelude::{Address, Eip1559TransactionRequest, NameOrAddress, U256};
use ethers::prelude::{Middleware, Signature, TxHash};
use ethers::types::H160;
use eyre::eyre;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

pub struct WalletconnectClient {
    pub client: Option<defi_wallet_connect::Client>,
    pub rt: tokio::runtime::Runtime, // need to use the same runtime, otherwise c++ side crash
}

async fn restore_client(contents: String) -> Result<Client> {
    if contents.is_empty() {
        anyhow::bail!("session info is empty");
    }

    let session: SessionInfo = serde_json::from_str(&contents)?;
    let client = Client::restore(session).await?;
    Ok(client)
}

async fn save_client(client: &Client) -> Result<String> {
    let session = client.get_session_info().await?;
    let session_info = serde_json::to_string(&session)?;
    Ok(session_info)
}

// description: "Defi WalletConnect example."
// url: "http://localhost:8080/"
// name: "Defi WalletConnect Web3 Example"
// chain_id: 25
async fn new_client(
    description: String,
    url: String,
    icon_urls: &[String],
    name: String,
    chain_id: u64,
) -> Result<Client> {
    // convert string array to url array
    let mut icons: Vec<Url> = Vec::new();
    for icon in icon_urls {
        icons.push(icon.parse()?);
    }
    let chain_id = match chain_id {
        0 => None,
        _ => Some(chain_id),
    };
    let client = Client::new(
        Metadata {
            description,
            url: url.parse()?,
            icons,
            name,
        },
        chain_id,
    )
    .await?;
    Ok(client)
}

pub fn walletconnect_restore_client(
    rt: &mut tokio::runtime::Runtime,
    session_info: String,
) -> Result<Client> {
    let res = rt.block_on(restore_client(session_info))?;
    Ok(res)
}

pub fn walletconnect_save_client(
    rt: &mut tokio::runtime::Runtime,
    client: &Client,
) -> Result<String> {
    let res = rt.block_on(save_client(client))?;
    Ok(res)
}

// description: "Defi WalletConnect example."
// url: "http://localhost:8080/".parse().expect("url")
// icons: vec![]
// name: "Defi WalletConnect Web3 Example",
pub fn walletconnect_new_client(
    rt: &mut tokio::runtime::Runtime,
    description: String,
    url: String,
    icon_urls: &[String],
    name: String,
    chain_id: u64,
) -> Result<Client> {
    let res = rt.block_on(new_client(description, url, icon_urls, name, chain_id))?;
    Ok(res)
}

fn convert_session_info(
    sessioninfo: &SessionInfo,
) -> eyre::Result<UniquePtr<WalletConnectSessionInfo>> {
    let mut cppsessioninfo = crate::ffi::new_walletconnect_sessioninfo();
    cppsessioninfo
        .pin_mut()
        .set_connected(sessioninfo.connected);

    let chain_id = match sessioninfo.chain_id {
        Some(id) => id.to_string(),
        None => "".to_string(),
    };
    cppsessioninfo.pin_mut().set_chainid(chain_id);

    let accountstrings = sessioninfo
        .accounts
        .iter()
        .map(|account| format!("{account:#x}"))
        .collect();
    cppsessioninfo.pin_mut().set_accounts(accountstrings);

    cppsessioninfo
        .pin_mut()
        .set_bridge(sessioninfo.bridge.to_string());

    cppsessioninfo
        .pin_mut()
        .set_key(format!("0x{}", hex::encode(sessioninfo.key.as_ref())));

    cppsessioninfo
        .pin_mut()
        .set_clientid(sessioninfo.client_id.to_string());
    cppsessioninfo
        .pin_mut()
        .set_clientmeta(serde_json::to_string(&sessioninfo.client_meta)?);

    cppsessioninfo
        .pin_mut()
        .set_peerid(match sessioninfo.peer_id.as_ref() {
            Some(id) => id.to_string(),
            None => "".to_string(),
        });

    cppsessioninfo
        .pin_mut()
        .set_peermeta(match sessioninfo.peer_meta.as_ref() {
            Some(meta) => serde_json::to_string(&meta)?,
            None => "".to_string(),
        });

    cppsessioninfo
        .pin_mut()
        .set_handshaketopic(sessioninfo.handshake_topic.to_string());

    Ok(cppsessioninfo)
}

async fn setup_callback(
    client: &mut Client,
    cppcallback: UniquePtr<WalletConnectCallback>,
) -> anyhow::Result<tokio::task::JoinHandle<eyre::Result<()>>> {
    client
        .run_callback(Box::new(
            move |message: ClientChannelMessage| -> eyre::Result<()> {
                match message.state {
                    ClientChannelMessageType::Connected => {
                        if let Some(info) = message.session {
                            let sessioninfo = convert_session_info(&info)?;
                            if let Some(myref) = sessioninfo.as_ref() {
                                cppcallback.onConnected(myref);
                                Ok(())
                            } else {
                                Err(eyre!("no session info"))
                            }
                        } else {
                            Err(eyre!("no session info"))
                        }
                    }
                    ClientChannelMessageType::Disconnected => {
                        if let Some(info) = message.session {
                            let sessioninfo = convert_session_info(&info)?;
                            if let Some(myref) = sessioninfo.as_ref() {
                                cppcallback.onDisconnected(myref);
                                Ok(())
                            } else {
                                Err(eyre!("no session info"))
                            }
                        } else {
                            Err(eyre!("no session info"))
                        }
                    }
                    ClientChannelMessageType::Connecting => {
                        if let Some(info) = &message.session {
                            let sessioninfo = convert_session_info(info)?;
                            if let Some(myref) = sessioninfo.as_ref() {
                                cppcallback.onConnecting(myref);
                                Ok(())
                            } else {
                                Err(eyre!("no session info"))
                            }
                        } else {
                            Err(eyre!("no session info"))
                        }
                    }
                    ClientChannelMessageType::Updated => {
                        if let Some(info) = &message.session {
                            let sessioninfo = convert_session_info(info)?;
                            if let Some(myref) = sessioninfo.as_ref() {
                                cppcallback.onUpdated(myref);
                                Ok(())
                            } else {
                                Err(eyre!("no session info"))
                            }
                        } else {
                            Err(eyre!("no session info"))
                        }
                    }
                } // end of match
            },
        ))
        .await
        .map_err(|e| anyhow!("{:?}", e))
}

async fn sign_typed_tx(
    client: Client,
    tx: &TypedTransaction,
    address: Address,
) -> Result<Signature> {
    let middleware = WCMiddleware::new(client);
    let signature = middleware.sign_transaction(tx, address).await?;
    Ok(signature)
}

async fn send_typed_tx(client: Client, tx: TypedTransaction, address: Address) -> Result<TxHash> {
    let middleware = WCMiddleware::new(client).with_sender(address);
    let receipt = middleware.send_transaction(tx, None).await?.tx_hash();
    Ok(receipt)
}

#[derive(Serialize, Deserialize)]
enum ContractAction {
    ContractApproval(defi_wallet_core_common::ContractApproval),
    ContractTransfer(defi_wallet_core_common::ContractTransfer),
}

impl WalletconnectClient {
    /// sign a message
    pub fn sign_personal_blocking(
        &mut self,
        message: String,
        address: [u8; 20],
    ) -> Result<Vec<u8>> {
        if let Some(client) = self.client.as_mut() {
            let signeraddress = Address::from_slice(&address);

            let result = self
                .rt
                .block_on(client.personal_sign(&message, &signeraddress))
                .map_err(|e| anyhow!("sign_personal error {}", e.to_string()))?;

            Ok(result.to_vec())
        } else {
            anyhow::bail!("no client");
        }
    }

    pub fn setup_callback_blocking(
        &mut self,
        usercallback: UniquePtr<WalletConnectCallback>,
    ) -> Result<()> {
        if let Some(client) = self.client.as_mut() {
            self.rt.block_on(async move {
                // FIXME handle the join_handle, or pass to c++ side
                let _join_handle = setup_callback(client, usercallback).await?;
                Ok(())
            })
        } else {
            anyhow::bail!("no client");
        }
    }

    /// ensure session, if session does not exist, create a new session
    pub fn ensure_session_blocking(
        self: &mut WalletconnectClient,
    ) -> Result<crate::ffi::WalletConnectEnsureSessionResult> {
        let mut ret = crate::ffi::WalletConnectEnsureSessionResult {
            addresses: Vec::new(),
            chain_id: 0,
        };
        if let Some(client) = self.client.as_mut() {
            let result: (Vec<Address>, u64) = self
                .rt
                .block_on(client.ensure_session())
                .map_err(|e| anyhow!("ensure_session error {}", e.to_string()))?;

            ret.addresses = result
                .0
                .iter()
                .map(|x| crate::ffi::WalletConnectAddress { address: x.0 })
                .collect();
            ret.chain_id = result.1;

            Ok(ret)
        } else {
            anyhow::bail!("no client");
        }
    }

    /// get connection string for qrcode display
    pub fn get_connection_string(&mut self) -> Result<String> {
        if let Some(client) = self.client.as_mut() {
            let result = self
                .rt
                .block_on(client.get_connection_string())
                .map_err(|e| anyhow!("get_connection_string error {}", e.to_string()))?;

            Ok(result)
        } else {
            anyhow::bail!("no client");
        }
    }

    /// save session to string which can be written to file
    pub fn save_client(&mut self) -> Result<String> {
        if let Some(client) = self.client.as_ref() {
            let result = walletconnect_save_client(&mut self.rt, client)?;
            Ok(result)
        } else {
            anyhow::bail!("no client");
        }
    }

    /// print uri(qrcode) for debugging
    pub fn print_uri(&mut self) -> Result<String> {
        if let Some(client) = self.client.as_ref() {
            let result = self
                .rt
                .block_on(client.get_session_info())
                .map_err(|e| anyhow!("get_sesion_info error {}", e.to_string()))?;
            result.uri().print_qr_uri();
            Ok(result.uri().as_url().as_str().into())
        } else {
            anyhow::bail!("no client");
        }
    }

    /// build cronos(eth) eip155 transaction
    pub fn sign_eip155_transaction_blocking(
        &mut self,
        userinfo: &crate::ffi::WalletConnectTxEip155,
        address: [u8; 20],
    ) -> Result<Vec<u8>> {
        if self.client.is_none() {
            anyhow::bail!("no client");
        }

        let client = self
            .client
            .as_ref()
            .ok_or_else(|| anyhow!("get walllet-connect client error"))?;
        let signeraddress = Address::from_slice(&address);

        let mut tx = Eip1559TransactionRequest::new();

        if !userinfo.to.is_empty() {
            tx = tx.to(NameOrAddress::Address(Address::from_str(&userinfo.to)?));
        }
        if !userinfo.data.is_empty() {
            tx = tx.data(userinfo.data.as_slice().to_vec());
        }
        if !userinfo.common.gas_limit.is_empty() {
            tx = tx.gas(U256::from_dec_str(&userinfo.common.gas_limit)?);
        }
        if !userinfo.common.gas_price.is_empty() {
            tx = tx
                .max_priority_fee_per_gas(U256::from_dec_str(&userinfo.common.gas_price)?)
                .max_fee_per_gas(U256::from_dec_str(&userinfo.common.gas_price)?);
        }
        if !userinfo.common.nonce.is_empty() {
            tx = tx.nonce(U256::from_dec_str(&userinfo.common.nonce)?);
        }
        if !userinfo.common.chainid == 0 {
            tx = tx.chain_id(userinfo.common.chainid);
        }
        if !userinfo.value.is_empty() {
            tx = tx.value(U256::from_dec_str(&userinfo.value)?);
        }
        let newclient = client.clone();
        let typedtx = TypedTransaction::Eip1559(tx);

        let sig = self
            .rt
            .block_on(sign_typed_tx(newclient, &typedtx, signeraddress))
            .map_err(|e| anyhow!("sign_typed_transaction error {}", e.to_string()))?;

        let signed_tx = &typedtx.rlp_signed(&sig);
        Ok(signed_tx.to_vec())
    }

    /// send cronos(eth) eip155 transaction
    pub fn send_eip155_transaction_blocking(
        &mut self,
        userinfo: &crate::ffi::WalletConnectTxEip155,
        address: [u8; 20],
    ) -> Result<Vec<u8>> {
        if self.client.is_none() {
            anyhow::bail!("no client");
        }

        let client = self
            .client
            .as_ref()
            .ok_or_else(|| anyhow!("get walllet-connect client error"))?;
        let signeraddress = Address::from_slice(&address);

        let mut tx = Eip1559TransactionRequest::new();

        if !userinfo.to.is_empty() {
            tx = tx.to(NameOrAddress::Address(Address::from_str(&userinfo.to)?));
        }
        if !userinfo.data.is_empty() {
            tx = tx.data(userinfo.data.as_slice().to_vec());
        }
        if !userinfo.common.gas_limit.is_empty() {
            tx = tx.gas(U256::from_dec_str(&userinfo.common.gas_limit)?);
        }
        if !userinfo.common.gas_price.is_empty() {
            tx = tx
                .max_priority_fee_per_gas(U256::from_dec_str(&userinfo.common.gas_price)?)
                .max_fee_per_gas(U256::from_dec_str(&userinfo.common.gas_price)?);
        }
        if !userinfo.common.nonce.is_empty() {
            tx = tx.nonce(U256::from_dec_str(&userinfo.common.nonce)?);
        }
        if !userinfo.common.chainid == 0 {
            tx = tx.chain_id(userinfo.common.chainid);
        }
        if !userinfo.value.is_empty() {
            tx = tx.value(U256::from_dec_str(&userinfo.value)?);
        }

        let newclient = client.clone();
        let typedtx = TypedTransaction::Eip1559(tx);

        let tx_bytes = self
            .rt
            .block_on(send_typed_tx(newclient, typedtx, signeraddress))
            .map_err(|e| anyhow!("send_typed_transaction error {}", e.to_string()))?;

        Ok(tx_bytes.0.to_vec())
    }

    fn get_signed_tx_raw_bytes(
        &self,
        newclient: Client,
        signeraddress: H160,
        typedtx: &mut TypedTransaction,
        common: &WalletConnectTxCommon,
    ) -> Result<Vec<u8>> {
        let mynonce = U256::from_dec_str(&common.nonce)?;
        if !mynonce.is_zero() {
            typedtx.set_nonce(mynonce);
        }
        typedtx.set_from(signeraddress);
        if !common.chainid == 0 {
            typedtx.set_chain_id(common.chainid);
        }
        if !common.gas_limit.is_empty() {
            typedtx.set_gas(U256::from_dec_str(&common.gas_limit)?);
        }
        if !common.gas_price.is_empty() {
            typedtx.set_gas_price(U256::from_dec_str(&common.gas_price)?);
        }

        let sig = self
            .rt
            .block_on(sign_typed_tx(newclient, typedtx, signeraddress))
            .map_err(|e| anyhow!("sign_typed_transaction error {}", e.to_string()))?;

        let signed_tx = &typedtx.rlp_signed(&sig);
        Ok(signed_tx.to_vec())
    }

    fn get_sent_tx_raw_bytes(
        &self,
        newclient: Client,
        signeraddress: H160,
        typedtx: &mut TypedTransaction,
        common: &WalletConnectTxCommon,
    ) -> Result<Vec<u8>> {
        let mynonce = U256::from_dec_str(&common.nonce)?;
        if !mynonce.is_zero() {
            typedtx.set_nonce(mynonce);
        }
        typedtx.set_from(signeraddress);
        if !common.chainid == 0 {
            typedtx.set_chain_id(common.chainid);
        }
        if !common.gas_limit.is_empty() {
            typedtx.set_gas(U256::from_dec_str(&common.gas_limit)?);
        }
        if !common.gas_price.is_empty() {
            typedtx.set_gas_price(U256::from_dec_str(&common.gas_price)?);
        }

        let tx_bytes = self
            .rt
            .block_on(send_typed_tx(newclient, typedtx.clone(), signeraddress))
            .map_err(|e| anyhow!("send_typed_transaction error {}", e.to_string()))?;

        Ok(tx_bytes.0.to_vec())
    }

    pub fn sign_transaction(
        &mut self,
        eip1559_transaction_request: String,
        address: [u8; 20],
    ) -> Result<Vec<u8>> {
        if self.client.is_none() {
            anyhow::bail!("no client");
        }

        let client = self
            .client
            .as_ref()
            .ok_or_else(|| anyhow!("get walllet-connect client error"))?;
        let signeraddress = Address::from_slice(&address);

        // parse json string transaction_info to TransactionRequest
        let tx: Eip1559TransactionRequest = serde_json::from_str(&eip1559_transaction_request)?;
        let typedtx = TypedTransaction::Eip1559(tx);

        let newclient = client.clone();
        let sig = self
            .rt
            .block_on(sign_typed_tx(newclient, &typedtx, signeraddress))
            .map_err(|e| anyhow!("sign_typed_transaction error {}", e.to_string()))?;

        let signed_tx = &typedtx.rlp_signed(&sig);
        Ok(signed_tx.to_vec())
    }

    pub fn send_transaction(
        &mut self,
        eip1559_transaction_request: String,
        address: [u8; 20],
    ) -> Result<Vec<u8>> {
        if self.client.is_none() {
            anyhow::bail!("no client");
        }

        let client = self
            .client
            .as_ref()
            .ok_or_else(|| anyhow!("get walllet-connect client error"))?;
        let signeraddress = Address::from_slice(&address);

        // parse json string transaction_info to TransactionRequest
        let tx: Eip1559TransactionRequest = serde_json::from_str(&eip1559_transaction_request)?;
        let typedtx = TypedTransaction::Eip1559(tx);

        let newclient = client.clone();
        let tx_bytes = self
            .rt
            .block_on(send_typed_tx(newclient, typedtx, signeraddress))
            .map_err(|e| anyhow!("send_typed_transaction error {}", e.to_string()))?;

        Ok(tx_bytes.0.to_vec())
    }

    pub fn sign_contract_transaction(
        &mut self,
        contract_action: String,
        common: &WalletConnectTxCommon,
        address: [u8; 20],
    ) -> Result<Vec<u8>> {
        if self.client.is_none() {
            anyhow::bail!("no client");
        }
        let signeraddress = Address::from_slice(&address);
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| anyhow!("get walllet-connect client error"))?;
        let newclient = client.clone();

        let action: ContractAction = serde_json::from_str(&contract_action)?;
        // parse json string transaction_info to TransactionRequest
        // let tx: ContractTransfer = serde_json::from_str(&contract_transaction_info)?;

        let mut typedtx = match action {
            ContractAction::ContractApproval(approval) => {
                self.rt
                    .block_on(defi_wallet_core_common::construct_contract_approval_tx(
                        approval,
                        defi_wallet_core_common::EthNetwork::Custom {
                            chain_id: common.chainid,
                            legacy: false,
                        },
                        common.web3api_url.as_str(),
                    ))?
            }
            ContractAction::ContractTransfer(transfer) => {
                self.rt
                    .block_on(defi_wallet_core_common::construct_contract_transfer_tx(
                        transfer,
                        defi_wallet_core_common::EthNetwork::Custom {
                            chain_id: common.chainid,
                            legacy: false,
                        },
                        // TODO unnessary for walletconnect
                        common.web3api_url.as_str(),
                    ))?
            }
        };

        let tx = self.get_signed_tx_raw_bytes(newclient, signeraddress, &mut typedtx, common)?;
        Ok(tx.to_vec())
    }

    pub fn send_contract_transaction(
        &mut self,
        contract_action: String,
        common: &WalletConnectTxCommon,
        address: [u8; 20],
    ) -> Result<Vec<u8>> {
        if self.client.is_none() {
            anyhow::bail!("no client");
        }
        let signeraddress = Address::from_slice(&address);
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| anyhow!("get walllet-connect client error"))?;
        let newclient = client.clone();

        let action: ContractAction = serde_json::from_str(&contract_action)?;
        // parse json string transaction_info to TransactionRequest
        // let tx: ContractTransfer = serde_json::from_str(&contract_transaction_info)?;

        let mut typedtx = match action {
            ContractAction::ContractApproval(approval) => {
                self.rt
                    .block_on(defi_wallet_core_common::construct_contract_approval_tx(
                        approval,
                        defi_wallet_core_common::EthNetwork::Custom {
                            chain_id: common.chainid,
                            legacy: false,
                        },
                        common.web3api_url.as_str(),
                    ))?
            }
            ContractAction::ContractTransfer(transfer) => {
                self.rt
                    .block_on(defi_wallet_core_common::construct_contract_transfer_tx(
                        transfer,
                        defi_wallet_core_common::EthNetwork::Custom {
                            chain_id: common.chainid,
                            legacy: false,
                        },
                        // TODO unnessary for walletconnect
                        common.web3api_url.as_str(),
                    ))?
            }
        };

        let tx = self.get_sent_tx_raw_bytes(newclient, signeraddress, &mut typedtx, common)?;
        Ok(tx.to_vec())
    }
}
