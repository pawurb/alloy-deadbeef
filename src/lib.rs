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
use tracing_subscriber::EnvFilter;

pub static ONE_ETHER: U256 = uint!(1_000_000_000_000_000_000_U256);
pub static GWEI: U256 = uint!(1_000_000_000_U256);
pub static GWEI_I: u128 = 1_000_000_000;

#[derive(Clone, Debug, Default)]
pub struct DeadbeefFiller {
    pub wallet: EthereumWallet,
    pub prefix: String,
}

#[derive(Debug)]
pub struct TxValueFillable {
    value: U256,
}

impl<N: Network> TxFiller<N> for DeadbeefFiller {
    type Fillable = TxValueFillable;

    fn status(&self, _tx: &<N as Network>::TransactionRequest) -> FillerControlFlow {
        FillerControlFlow::Ready
    }
    fn fill_sync(&self, _tx: &mut SendableTx<N>) {}

    async fn fill(
        &self,
        fillable: Self::Fillable,
        mut tx: SendableTx<N>,
    ) -> TransportResult<SendableTx<N>> {
        if let Some(builder) = tx.as_mut_builder() {
            builder.set_value(fillable.value);
            dbg!(&builder);
        }

        Ok(tx)
    }

    async fn prepare<P, T>(
        &self,
        _provider: &P,
        tx: &<N as Network>::TransactionRequest,
    ) -> TransportResult<Self::Fillable>
    where
        P: Provider<T, N>,
        T: Transport + Clone,
    {
        let rpc_tx = TransactionRequest {
            from: tx.from(),
            to: Some(tx.to().into()),
            value: tx.value(),
            chain_id: tx.chain_id(),
            nonce: tx.nonce(),
            max_fee_per_gas: tx.max_fee_per_gas(),
            max_priority_fee_per_gas: tx.max_priority_fee_per_gas(),
            gas: tx.gas_limit(),
            access_list: tx.access_list().cloned(),
            ..Default::default()
        };

        let value = prefixed_tx_value(rpc_tx, self.wallet.clone(), &self.prefix)
            .await
            .unwrap();

        Ok(TxValueFillable { value })
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
        // select! {
        //     biased;
        //     _ = done.cancelled() => {
        //       break None;
        //     }
        //     _ = futures::future::ready(1) => {
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
        //     }
        // }
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

#[cfg(test)]

mod tests {
    use super::*;
    use alloy::{
        network::EthereumWallet,
        node_bindings::Anvil,
        primitives::{address, Address, U256},
        providers::{Provider, ProviderBuilder},
        rpc::types::TransactionRequest,
        signers::local::PrivateKeySigner,
    };

    const TX_PREFIX: &str = "de";

    #[tokio::test]
    async fn test_prefixed_tx_value() -> Result<()> {
        let anvil = Anvil::new().spawn();
        let account = anvil.addresses()[0];
        let private_key = anvil.keys()[0].clone();
        let wallet = EthereumWallet::from(PrivateKeySigner::from(private_key));

        let anvil_provider = ProviderBuilder::new()
            .filler(DeadbeefFiller {
                wallet: wallet.clone(),
                prefix: TX_PREFIX.to_string(),
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

        let tx_hash = res.transaction_hash;
        let tx_hash = format!("{:x}", &tx_hash);
        let tx_prefix = &tx_hash[..TX_PREFIX.len()];

        assert_eq!(tx_prefix.as_bytes(), "de".as_bytes());
        // let signer: Private
        Ok(())
    }
}
