use alloy::{
    network::EthereumWallet,
    node_bindings::Anvil,
    primitives::{address, Address, U256},
    providers::{Provider, ProviderBuilder},
    rpc::{client::ClientBuilder, types::TransactionRequest},
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

    let client = ClientBuilder::default().http(anvil.endpoint().parse()?);

    let anvil_provider = ProviderBuilder::new()
        .with_chain_id(provider.get_chain_id().await?)
        .on_client(client);

    let gas_price = anvil_provider.get_gas_price().await?;
    let nonce = anvil_provider.get_transaction_count(ME).await?;

    let signer: PrivateKeySigner = std::env::var("PRIVATE_KEY")?.parse()?;
    let wallet = EthereumWallet::from(signer);
    let chain_id = provider.get_chain_id().await?;

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

    let res = prefixed_tx(tx, wallet, "deade").await?;

    Ok(())
}
