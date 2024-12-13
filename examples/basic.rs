use alloy::{
    network::EthereumWallet,
    node_bindings::Anvil,
    primitives::U256,
    providers::{Provider, ProviderBuilder},
    rpc::types::TransactionRequest,
    signers::local::PrivateKeySigner,
};

use alloy_deadbeef::{prefixed_tx_value_token, DeadbeefFiller, GWEI_I};
use eyre::Result;
#[tokio::main]
async fn main() -> Result<()> {
    let anvil = Anvil::new().spawn();
    let account = anvil.addresses()[0];
    let private_key = anvil.keys()[0].clone();
    let wallet = EthereumWallet::from(PrivateKeySigner::from(private_key));

    let anvil_provider = ProviderBuilder::new()
        .filler(DeadbeefFiller {
            wallet: wallet.clone(),
            prefix: "dead".to_string(),
        })
        .wallet(wallet.clone())
        .on_http(anvil.endpoint().parse()?);

    let chain_id = anvil_provider.get_chain_id().await?;

    let nonce = anvil_provider.get_transaction_count(account).await?;
    let gas_price = anvil_provider.get_gas_price().await?;

    let tx = TransactionRequest {
        from: Some(account),
        to: Some(account.into()),
        value: Some(U256::ZERO),
        chain_id: Some(chain_id),
        nonce: Some(nonce),
        max_fee_per_gas: Some(gas_price * 110 / 100),
        max_priority_fee_per_gas: Some(GWEI_I),
        gas: Some(210000),
        ..Default::default()
    };

    let res = anvil_provider
        .send_transaction(tx)
        .await?
        .get_receipt()
        .await?;
    dbg!(&res.transaction_hash);

    Ok(())
}
