use alloy::{
    eips::eip2718::Encodable2718,
    network::{EthereumWallet, TransactionBuilder},
    primitives::{keccak256, FixedBytes, U256},
    rpc::types::TransactionRequest,
    uint,
};
use eyre::Result;
use std::{sync::Arc, thread::available_parallelism};
use tokio::sync::Notify;

pub static ONE_ETHER: U256 = uint!(1_000_000_000_000_000_000_U256);
pub static GWEI: U256 = uint!(1_000_000_000_U256);
pub static GWEI_I: u128 = 1_000_000_000;

pub struct PrefixResult {
    pub tx: Option<TransactionRequest>,
    pub iterations: u128,
}

pub async fn prefixed_tx(
    tx: TransactionRequest,
    wallet: EthereumWallet,
    prefix: &str,
) -> Result<TransactionRequest> {
    let max_cores: u128 = available_parallelism().unwrap().get() as u128;
    dbg!(&max_cores);
    let max_value = 16_u128.pow(prefix.len() as u32);

    let mut handles = vec![];

    let done = Arc::new(Notify::new());
    for i in 0..max_cores {
        let tx = tx.clone();
        let prefix = prefix.to_string();
        let wallet = wallet.clone();
        let done = done.clone();
        let handle = tokio::spawn(async move {
            search_tx_hash(
                tx,
                wallet,
                i * max_value / max_cores,
                prefix,
                &i.to_string(),
                done,
            )
            .await
            .unwrap()
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }

    todo!()
}

async fn search_tx_hash(
    tx: TransactionRequest,
    wallet: EthereumWallet,
    starting_input: u128,
    prefix: String,
    label: &str,
    done: Arc<Notify>,
) -> Result<Option<FixedBytes<32>>> {
    let mut iter = 0;
    let mut value = starting_input;
    let prefix = prefix.as_bytes();

    let tx_hash: Option<FixedBytes<32>> = loop {
        tokio::select! {
            _ = done.notified() => {
                break None;
            }
            else => {
              let tx = tx.clone();
              let next_value = tx.value.unwrap_or_default() + U256::from(value);
              let tx_hash = calculate_hash(tx, &wallet, next_value).await?;

              value += 1;
              iter += 1;

              if value % 10000 == 0 {
                  dbg!(label, value, iter);
              }

              let hash_str = format!("{:x}", &tx_hash);
              let hash_prefix = &hash_str[..prefix.len()];
              let first_hash_bytes = hash_prefix.as_bytes();
              if first_hash_bytes == prefix {
                  dbg!("found");
                  dbg!(hash_str);
                  done.notify_one();
                  break Some(tx_hash);
              }
            }
        }
    };

    Ok(tx_hash)
}

async fn calculate_hash(
    tx: TransactionRequest,
    wallet: &EthereumWallet,
    value: U256,
) -> Result<FixedBytes<32>> {
    let mut tx = tx;
    tx.value = Some(value);
    let tx_envelope = tx.build(&wallet).await?;
    let mut encoded_tx = vec![];
    tx_envelope.encode_2718(&mut encoded_tx);
    let tx_hash = keccak256(&encoded_tx);
    Ok(tx_hash)
}
