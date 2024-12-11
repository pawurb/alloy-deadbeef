use alloy::{
    eips::eip2718::Encodable2718,
    network::{EthereumWallet, TransactionBuilder},
    node_bindings::Anvil,
    primitives::{address, keccak256},
    providers::{Provider, ProviderBuilder},
    rpc::types::TransactionRequest,
    signers::local::PrivateKeySigner,
    uint,
};
use alloy_deadbeef::GWEI_I;
use eyre::Result;
#[tokio::main]
async fn main() -> Result<()> {
    dbg!("basic");
    let me = address!("82F8f740fD0B74ccDC7404dEe96fE1c9A9B7445C");
    // let rpc = "https://eth.merkle.io";
    let rpc = std::env::var("RPC_URL").unwrap();
    let provider = ProviderBuilder::new().on_http(rpc.parse()?);

    let anvil = Anvil::new().fork(rpc).spawn();

    let signer: PrivateKeySigner = std::env::var("PRIVATE_KEY")?.parse()?;
    let wallet = EthereumWallet::from(signer);
    let anvil_provider = ProviderBuilder::new()
        .with_chain_id(42161)
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
        chain_id: Some(uint!(42161)),
        max_fee_per_gas: Some(gas_price * 120 / 100),
        max_priority_fee_per_gas: Some(GWEI_I),
        gas: Some(210000),
        ..Default::default()
    };
    let tx_envelope = tx.build(&wallet).await?;
    let mut encoded_tx = vec![];
    tx_envelope.encode_2718(&mut encoded_tx);

    // let real_tx = provider.send_raw_transaction(&encoded_tx).await?;
    // dbg!(real_tx);

    // let anvil_receipt = anvil_provider
    //     .send_raw_transaction(&encoded_tx)
    //     .await?
    //     .get_receipt()
    //     .await?;
    let encoded = keccak256(&encoded_tx);
    dbg!(encoded);
    // dbg!(anvil_receipt.transaction_hash);

    Ok(())
}
