use alloy::{
    network::EthereumWallet,
    node_bindings::Anvil,
    primitives::{address, Address, U256},
    providers::{Provider, ProviderBuilder},
    rpc::types::TransactionRequest,
    signers::local::PrivateKeySigner,
};

static ME: Address = address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266");

use alloy_deadbeef::{prefixed_tx_value, DeadbeefFiller, GWEI_I};
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
        .with_recommended_fillers()
        .wallet(wallet.clone())
        .filler(DeadbeefFiller)
        .on_http(anvil.endpoint().parse()?);

    dbg!(anvil_provider.get_chain_id().await?);

    let nonce = anvil_provider.get_transaction_count(ME).await?;
    let gas_price = anvil_provider.get_gas_price().await?;

    let tx = TransactionRequest {
        from: Some(ME),
        to: Some(ME.into()),
        value: Some(U256::ZERO),
        chain_id: Some(chain_id),
        nonce: Some(nonce),
        max_fee_per_gas: Some(gas_price * 110 / 100),
        max_priority_fee_per_gas: Some(GWEI_I),
        gas: Some(210000),
        ..Default::default()
    };

    let res = prefixed_tx_value(tx, wallet, "dea").await?;
    // dbg!(&res);
    dbg!("done");

    let res = anvil_provider
        .send_transaction(res)
        .await?
        .get_receipt()
        .await?;
    dbg!(res.transaction_hash);

    Ok(())
}
