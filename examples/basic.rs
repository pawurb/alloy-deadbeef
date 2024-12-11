use alloy::{
    eips::eip2718::Encodable2718,
    network::{EthereumWallet, TransactionBuilder},
    node_bindings::Anvil,
    primitives::{address, keccak256, Address, Bytes, B256, U256},
    providers::{Provider, ProviderBuilder},
    rpc::types::TransactionRequest,
    signers::local::PrivateKeySigner,
    uint,
};
use std::thread::available_parallelism;

static ME: Address = address!("82F8f740fD0B74ccDC7404dEe96fE1c9A9B7445C");

use alloy_deadbeef::GWEI_I;
use eyre::Result;
#[tokio::main]
async fn main() -> Result<()> {
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
    let nonce = anvil_provider.get_transaction_count(ME).await?;

    let mut iter: u128 = 0;

    let prefix = "deadbeef".as_bytes();
    let max_cores: u128 = available_parallelism().unwrap().get() as u128;
    dbg!(&max_cores);

    let max_value = 16_u128.pow(8);

    let mut handles = vec![];
    for i in 0..max_cores {
        let handle = tokio::spawn(async move {
            search_tx_hash(i * max_value / max_cores, prefix, nonce, gas_price)
                .await
                .unwrap();
        });
        handles.push(handle);
    }
    // dbg!(anvil_receipt.transaction_hash);

    for handle in handles {
        handle.await?;
    }

    Ok(())
}

async fn search_tx_hash(
    starting_input: u128,
    prefix: &[u8],
    nonce: u64,
    gas_price: u128,
) -> Result<()> {
    let mut iter = starting_input;
    dbg!(starting_input);
    let signer: PrivateKeySigner = std::env::var("PRIVATE_KEY")?.parse()?;
    let wallet = EthereumWallet::from(signer);
    loop {
        let tx = TransactionRequest {
            from: Some(ME),
            to: Some(ME.into()),
            value: Some(U256::from(iter)),
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
        let tx_hash = keccak256(&encoded_tx);

        iter += 1;

        if iter % 100000 == 0 {
            dbg!(iter);
        }

        let hash_str = format!("{:x}", &tx_hash);
        let hash_prefix = &hash_str[..prefix.len()];
        let first_hash_bytes = hash_prefix.as_bytes();
        // let prefix_str = format!("{:x}", &prefix_bytes);

        // dbg!(
        //     &hash_str,
        //     &prefix_str,
        //     &prefix_bytes,
        //     &first_hash_bytes,
        //     &hash_prefix
        // );
        if first_hash_bytes == prefix.to_vec() {
            dbg!("found");
            dbg!(hash_str);
            break;
        }
    }

    std::process::exit(0);
    Ok(())
}
