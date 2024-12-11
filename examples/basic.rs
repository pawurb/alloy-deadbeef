use alloy::{
    network::{EthereumWallet, TransactionBuilder},
    node_bindings::Anvil,
    primitives::address,
    providers::{Provider, ProviderBuilder},
    rpc::types::TransactionRequest,
    signers::local::PrivateKeySigner,
    uint,
};
use eyre::Result;
#[tokio::main]
async fn main() -> Result<()> {
    dbg!("basic");
    let me = address!("82F8f740fD0B74ccDC7404dEe96fE1c9A9B7445C");
    // let rpc = "https://eth.merkle.io";
    let rpc = std::env::var("RPC_URL").unwrap();
    // let provider = ProviderBuilder::new().on_http(rpc.parse()?);

    let anvil = Anvil::new().fork(rpc).spawn();

    let signer: PrivateKeySigner = std::env::var("PRIVATE_KEY")?.parse()?;
    let wallet = EthereumWallet::from(signer);
    let anvil_provider = ProviderBuilder::new()
        // .wallet(wallet)
        .on_http(anvil.endpoint().parse()?);

    let gas_price = anvil_provider.get_gas_price().await?;
    dbg!(gas_price);
    let nonce = anvil_provider.get_transaction_count(me).await?;

    let tx = TransactionRequest {
        from: Some(me),
        to: Some(me.into()),
        value: Some(uint!(1_U256)),
        nonce: Some(nonce),
        max_fee_per_gas: Some(gas_price),
        max_priority_fee_per_gas: Some(gas_price / 10),
        gas: Some(21000),
        ..Default::default()
    };
    let tx_envelope = tx.build(&wallet).await?;
    dbg!(tx_envelope);

    // let tx = anvil_provider.send_transaction(tx).await?;
    // dbg!(tx);

    Ok(())
}
