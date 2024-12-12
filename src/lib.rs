use alloy::{
    eips::eip2718::Encodable2718,
    network::{EthereumWallet, TransactionBuilder},
    primitives::{keccak256, FixedBytes, U256},
    rpc::types::TransactionRequest,
    uint,
};
use eyre::Result;
use futures::future::select_all;
use futures::future::{AbortHandle, Abortable, Aborted};
use std::{thread::available_parallelism, time::Duration};
use tokio::{select, time::Instant};
use tokio_util::sync::CancellationToken;
use tracing::info;
use tracing::subscriber::set_global_default;
use tracing_subscriber::{fmt, EnvFilter};

pub static ONE_ETHER: U256 = uint!(1_000_000_000_000_000_000_U256);
pub static GWEI: U256 = uint!(1_000_000_000_U256);
pub static GWEI_I: u128 = 1_000_000_000;

pub async fn prefixed_tx(
    tx: TransactionRequest,
    wallet: EthereumWallet,
    prefix: &str,
) -> Result<TransactionRequest> {
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .finish();
    _ = set_global_default(subscriber);

    info!("Looking for '0x{prefix}' tx prefix");

    let max_cores: u128 = available_parallelism().unwrap().get() as u128;
    dbg!(&max_cores);
    let max_value = 16_u128.pow(prefix.len() as u32);
    let mut source_tx = tx.clone();

    let mut handles = vec![];
    let done = CancellationToken::new();

    for i in 0..max_cores {
        let tx = tx.clone();
        let prefix = prefix.to_string();
        let wallet = wallet.clone();
        let done = done.clone();
        let handle = tokio::spawn(async move {
            search_tx_hash(tx, wallet, done, i * max_value / max_cores, prefix)
                .await
                .unwrap()
        });
        handles.push(handle);
    }

    let (value, _index, remaining) = select_all(handles).await;

    done.cancel();
    // for handle in remaining {
    //     dbg!("aborting");
    //     handle.abort();
    //     let _ = handle.await;
    //     dbg!("awaited");
    // }

    source_tx.value = value.unwrap();

    Ok(source_tx)
}

async fn search_tx_hash(
    tx: TransactionRequest,
    wallet: EthereumWallet,
    done: CancellationToken,
    starting_input: u128,
    prefix: String,
) -> Result<Option<U256>> {
    let mut value = starting_input;
    let prefix = prefix.as_bytes();

    let result: Option<U256> = loop {
        // let start = measure_start("loop");
        select! {
            _ = done.cancelled() => {
              break None;
            }
            _ = futures::future::ready(1) => {
              let tx = tx.clone();
              let next_value = tx.value.unwrap_or_default() + U256::from(value);
              let tx_hash = calculate_hash(tx, &wallet, next_value).await?;

              value += 1;

              let hash_str = format!("{:x}", &tx_hash);
              let hash_prefix = &hash_str[..prefix.len()];
              let first_hash_bytes = hash_prefix.as_bytes();
              if first_hash_bytes == prefix {
                  info!("Found matching tx hash: {tx_hash}");
                  break Some(next_value);
              }
            }
        }
        // measure_end(start);
    };

    Ok(result)
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

pub fn measure_start(label: &str) -> (String, Instant) {
    (label.to_string(), Instant::now())
}

pub fn measure_end(start: (String, Instant)) -> Duration {
    let elapsed = start.1.elapsed();
    println!("Elapsed: {:.2?} for '{}'", elapsed, start.0);
    elapsed
}
