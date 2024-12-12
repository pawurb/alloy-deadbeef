use alloy::{
    eips::eip2718::Encodable2718,
    network::{EthereumWallet, Network, TransactionBuilder},
    primitives::{keccak256, FixedBytes, U256},
    providers::{
        fillers::{FillerControlFlow, TxFiller},
        Provider, SendableTx,
    },
    rpc::types::TransactionRequest,
    transports::{Transport, TransportResult},
    uint,
};
use eyre::Result;
use futures::future::{AbortHandle, Abortable, Aborted};
use futures::{future::select_all, FutureExt};
use std::{thread::available_parallelism, time::Duration};
use tokio::{select, time::Instant};
use tokio_util::sync::CancellationToken;
use tracing::info;
use tracing::subscriber::set_global_default;
use tracing_subscriber::{EnvFilter};

pub static ONE_ETHER: U256 = uint!(1_000_000_000_000_000_000_U256);
pub static GWEI: U256 = uint!(1_000_000_000_U256);
pub static GWEI_I: u128 = 1_000_000_000;

#[derive(Clone, Debug, Default)]
pub struct DeadbeefFiller {
  wallet: EthereumWallet,
};

#[derive(Debug)]
pub struct TxValueFillable {
    value: U256,
}

impl<N: Network> TxFiller<N> for DeadbeefFiller {
    type Fillable = TxValueFillable;

    fn status(&self, _tx: &<N as Network>::TransactionRequest) -> FillerControlFlow {
        dbg!("status");
        FillerControlFlow::Ready
    }
    fn fill_sync(&self, _tx: &mut SendableTx<N>) {}
    async fn fill(
        &self,
        fillable: Self::Fillable,
        mut tx: SendableTx<N>,
    ) -> TransportResult<SendableTx<N>> {
        dbg!("fill");
        dbg!(&fillable);
        Ok(tx)
    }

    async fn prepare<P, T>(
        &self,
        provider: &P,
        tx: &<N as Network>::TransactionRequest,
    ) -> TransportResult<Self::Fillable>
    where
        P: Provider<T, N>,
        T: Transport + Clone,
    {
        dbg!("prepare");
        let value = prefixed_tx_value(tx.clone().into(), provider.wallet().clone(), "dead")
            .await
            .unwrap();
        dbg!(&tx);
        Ok(TxValueFillable {
            value: tx.value().unwrap_or_default(),
        })
    }
}

pub async fn prefixed_tx_value(
    tx: TransactionRequest,
    wallet: EthereumWallet,
    prefix: &str,
) -> Result<U256> {
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .finish();
    _ = set_global_default(subscriber);

    let max_cores: u128 = available_parallelism().unwrap().get() as u128;
    info!("Looking for '0x{prefix}' tx hash prefix with {max_cores} CPU cores");

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

    let (value, _index, _remaining) = select_all(handles).await;

    done.cancel();

    Ok(value.unwrap().unwrap())
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
            biased;
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
