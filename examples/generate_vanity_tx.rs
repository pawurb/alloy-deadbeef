use alloy::{
    network::EthereumWallet,
    primitives::{Address, U256},
    providers::{Provider, ProviderBuilder},
    rpc::types::TransactionRequest,
    signers::local::PrivateKeySigner,
};

use alloy_deadbeef::{DeadbeefFiller, GWEI_I};
use eyre::Result;
#[tokio::main]
async fn main() -> Result<()> {
    let account: Address = std::env::var("ACCOUNT")
        .expect("ACCOUNT must be set")
        .parse()?;
    let private_key: PrivateKeySigner = std::env::var("PRIVATE_KEY")
        .expect("PRIVATE_KEY must be set")
        .parse()?;
    let wallet = EthereumWallet::from(private_key);
    let endpoint = std::env::var("ETH_RPC").expect("ETH_RPC must be set");

    let provider = ProviderBuilder::new()
        .wallet(wallet.clone())
        .on_http(endpoint.parse()?);

    let chain_id = provider.get_chain_id().await?;

    let nonce = provider.get_transaction_count(account).await?;
    let gas_price = provider.get_gas_price().await?;

    let tx = TransactionRequest {
        from: Some(account),
        to: Some(account.into()),
        value: Some(U256::ZERO),
        chain_id: Some(chain_id),
        nonce: Some(nonce),
        max_fee_per_gas: Some(gas_price * 2),
        max_priority_fee_per_gas: Some(GWEI_I),
        gas: Some(21000),
        ..Default::default()
    };

    let prefix = std::env::var("PREFIX").unwrap();
    let deadbeef = DeadbeefFiller::new(prefix, wallet.clone()).unwrap();

    let prefixed_tx = deadbeef.prefixed_tx(tx.clone()).await?;

    dbg!(prefixed_tx);

    Ok(())
}
