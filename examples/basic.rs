use alloy::{
    eips::eip2718::Encodable2718,
    network::{EthereumWallet, TransactionBuilder},
    node_bindings::Anvil,
    primitives::{address, keccak256, Address, Bytes, B256, U256},
    providers::{Provider, ProviderBuilder},
    rpc::{client::ClientBuilder, types::TransactionRequest},
    signers::local::PrivateKeySigner,
    uint,
};
use std::thread::available_parallelism;

static ME: Address = address!("82F8f740fD0B74ccDC7404dEe96fE1c9A9B7445C");

use alloy_deadbeef::{prefixed_tx, GWEI_I};
use eyre::Result;
#[tokio::main]
async fn main() -> Result<()> {
    // let rpc = "https://eth.merkle.io";
    let rpc = std::env::var("RPC_URL").unwrap();
    let provider = ProviderBuilder::new().on_http(rpc.parse()?);

    let anvil = Anvil::new().fork(rpc).spawn();

    let signer: PrivateKeySigner = std::env::var("PRIVATE_KEY")?.parse()?;
    let wallet = EthereumWallet::from(signer);
    let client = ClientBuilder::default().http(anvil.endpoint().parse()?);

    let anvil_provider = ProviderBuilder::new()
        .with_chain_id(42161)
        .on_client(client);

    let gas_price = anvil_provider.get_gas_price().await?;
    let nonce = anvil_provider.get_transaction_count(ME).await?;

    let tx = TransactionRequest {
        from: Some(ME),
        to: Some(ME.into()),
        value: Some(U256::from(1)),
        nonce: Some(nonce),
        chain_id: Some(uint!(42161)),
        max_fee_per_gas: Some(gas_price * 120 / 100),
        max_priority_fee_per_gas: Some(GWEI_I),
        gas: Some(210000),
        ..Default::default()
    };

    let tx_envelope = tx.build(&wallet).await?;

    let res = anvil_provider
        .send_tx_envelope(tx_envelope)
        .await?
        .get_receipt()
        .await?;
    dbg!("done");

    let tx = TransactionRequest {
        from: Some(ME),
        to: Some(ME.into()),
        value: Some(U256::ZERO),
        nonce: Some(nonce),
        chain_id: Some(uint!(42161)),
        max_fee_per_gas: Some(gas_price * 120 / 100),
        max_priority_fee_per_gas: Some(GWEI_I),
        gas: Some(210000),
        ..Default::default()
    };

    let res = prefixed_tx(tx, "dead")?;

    Ok(())
}
