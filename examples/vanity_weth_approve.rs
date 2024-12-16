use alloy::{
    network::EthereumWallet,
    node_bindings::Anvil,
    primitives::{address, Address, U256},
    providers::{Provider, ProviderBuilder},
    rpc::types::TransactionRequest,
    signers::local::PrivateKeySigner,
    sol,
    sol_types::SolCall,
};

use std::ops::Div;

use alloy_deadbeef::{DeadbeefFiller, GWEI_I};
use eyre::Result;

sol! {
    #[sol(rpc)]
    contract WETH {
        function approve(address spender, uint256 amount) public returns (bool);
    }
}

static WETH_ADDR: Address = address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");

#[tokio::main]
async fn main() -> Result<()> {
    let endpoint = std::env::var("ETH_RPC").expect("ETH_RPC env var not set");
    let anvil = Anvil::new().fork(endpoint).spawn();
    let account = anvil.addresses()[0];
    let private_key = anvil.keys()[0].clone();
    let wallet = EthereumWallet::from(PrivateKeySigner::from(private_key));

    let deadbeef = DeadbeefFiller::new("bee".to_string(), wallet.clone()).unwrap();

    let anvil_provider = ProviderBuilder::new()
        .wallet(wallet.clone())
        .on_http(anvil.endpoint().parse()?);

    let approve_input = WETH::approveCall {
        spender: WETH_ADDR,
        amount: U256::MAX.div(U256::from(2)),
    }
    .abi_encode();

    let chain_id = anvil_provider.get_chain_id().await?;
    let nonce = anvil_provider.get_transaction_count(account).await?;
    let gas_price = anvil_provider.get_gas_price().await?;

    let tx = TransactionRequest {
        from: Some(account),
        to: Some(account.into()),
        value: Some(U256::ZERO),
        chain_id: Some(chain_id),
        input: approve_input.into(),
        nonce: Some(nonce),
        max_fee_per_gas: Some(gas_price * 110 / 100),
        max_priority_fee_per_gas: Some(GWEI_I),
        gas: Some(21000),
        ..Default::default()
    };

    let tx = deadbeef.prefixed_tx(tx.clone()).await?;

    let res = anvil_provider
        .send_transaction(tx)
        .await?
        .get_receipt()
        .await?;

    println!("Sent transaction: {}", res.transaction_hash);

    Ok(())
}
