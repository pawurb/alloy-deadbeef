use alloy::{
    network::EthereumWallet,
    node_bindings::Anvil,
    primitives::{address, Address, U256},
    providers::{Provider, ProviderBuilder},
    rpc::types::TransactionRequest,
    signers::local::PrivateKeySigner,
};

static ME: Address = address!("82F8f740fD0B74ccDC7404dEe96fE1c9A9B7445C");

use alloy_deadbeef::{prefixed_tx, GWEI_I};
use eyre::Result;
#[tokio::main]
async fn main() -> Result<()> {
    let rpc = std::env::var("RPC_URL").unwrap();
    let provider = ProviderBuilder::new().on_http(rpc.parse()?);

    let anvil = Anvil::new().fork(rpc).spawn();
    let signer: PrivateKeySigner = std::env::var("PRIVATE_KEY")?.parse()?;
    let wallet = EthereumWallet::from(signer);
    let chain_id = provider.get_chain_id().await?;

    let anvil_provider = ProviderBuilder::new()
        .wallet(wallet.clone())
        .on_http(anvil.endpoint().parse()?);

    let gas_price = anvil_provider.get_gas_price().await?;
    let nonce = anvil_provider.get_transaction_count(ME).await?;

    let tx = TransactionRequest {
        from: Some(ME),
        to: Some(ME.into()),
        value: Some(U256::ZERO),
        nonce: Some(nonce),
        chain_id: Some(chain_id),
        max_fee_per_gas: Some(gas_price * 120 / 100),
        max_priority_fee_per_gas: Some(GWEI_I),
        gas: Some(210000),
        ..Default::default()
    };

    let res = prefixed_tx(tx, wallet, "dead").await?;

    let res = anvil_provider
        .send_transaction(res)
        .await?
        .get_receipt()
        .await?;
    dbg!(res.transaction_hash);

    Ok(())
}
