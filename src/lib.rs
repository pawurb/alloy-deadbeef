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
use eyre::Result;
use std::thread::available_parallelism;

pub static ONE_ETHER: U256 = uint!(1_000_000_000_000_000_000_U256);
pub static GWEI: U256 = uint!(1_000_000_000_U256);
pub static GWEI_I: u128 = 1_000_000_000;

pub fn prefixed_tx(tx: TransactionRequest, prefix: &str) -> Result<TransactionRequest> {
    let max_cores: u128 = available_parallelism().unwrap().get() as u128;
    dbg!(&max_cores);
    let max_value = 16_u128.pow(prefix.len() as u32);

    let mut handles = vec![];

    for i in 0..max_cores {
        let tx = tx.clone();
        let prefix = prefix.to_string();
        let handle = tokio::spawn(async move {
            search_tx_hash(tx, i * max_value / max_cores, prefix, &i.to_string())
                .await
                .unwrap();
        });
        handles.push(handle);
    }

    todo!()
}

async fn search_tx_hash(
    tx: TransactionRequest,
    starting_input: u128,
    prefix: String,
    label: &str,
) -> Result<()> {
    let mut iter = 0;
    let mut value = starting_input;
    dbg!(starting_input);
    let signer: PrivateKeySigner = std::env::var("PRIVATE_KEY")?.parse()?;
    let wallet = EthereumWallet::from(signer);
    let prefix = prefix.as_bytes();

    loop {
        let mut tx = tx.clone();
        tx.value = Some(U256::from(value));
        let tx_envelope = tx.build(&wallet).await?;
        let mut encoded_tx = vec![];
        tx_envelope.encode_2718(&mut encoded_tx);
        let tx_hash = keccak256(&encoded_tx);

        value += 1;
        iter += 1;

        if value % 1000000 == 0 {
            dbg!(label, value, iter);
        }

        let hash_str = format!("{:x}", &tx_hash);
        let hash_prefix = &hash_str[..prefix.len()];
        let first_hash_bytes = hash_prefix.as_bytes();
        if first_hash_bytes == prefix {
            dbg!("found");
            dbg!(hash_str);
            break;
        }
    }

    std::process::exit(0);
    Ok(())
}
