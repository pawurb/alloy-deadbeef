use alloy::{
    eips::eip2718::Encodable2718,
    network::{EthereumWallet, Network, TransactionBuilder},
    primitives::{keccak256, FixedBytes, U256},
    providers::{
        fillers::{FillerControlFlow, TxFiller},
        Provider, SendableTx,
    },
    rpc::types::{TransactionInput, TransactionRequest},
    transports::{Transport, TransportResult},
    uint,
};
use eyre::Result;
use futures::{future, future::select_all};
use std::thread::available_parallelism;
use tokio::{select, sync::broadcast};
use tracing::{error, info, subscriber::set_global_default};
use tracing_subscriber::EnvFilter;

pub static ONE_ETHER: U256 = uint!(1_000_000_000_000_000_000_U256);
pub static GWEI: U256 = uint!(1_000_000_000_U256);
pub static GWEI_I: u128 = 1_000_000_000;

#[derive(Clone, Debug)]
pub struct DeadbeefFiller {
    wallet: EthereumWallet,
    prefix: String,
    iteration_mode: IterationMode,
}

#[derive(Clone, Debug)]
pub enum IterationMode {
    Value,
    Gas,
}

impl DeadbeefFiller {
    pub fn new(prefix: String, wallet: EthereumWallet) -> Result<Self, &'static str> {
        if prefix.is_empty() {
            return Err("Prefix cannot be empty");
        }

        if !prefix.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err("Prefix contains non-hexadecimal characters");
        }
        let iteration_mode = if prefix.len() <= 4 {
            IterationMode::Gas
        } else {
            IterationMode::Value
        };

        Ok(Self {
            wallet,
            prefix,
            iteration_mode,
        })
    }
}

#[derive(Debug)]
pub enum TxFillable {
    Value { value: U256 },
    Gas { gas: u64 },
}

impl<N: Network> TxFiller<N> for DeadbeefFiller {
    type Fillable = TxFillable;

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
            match fillable {
                TxFillable::Value { value } => builder.set_value(value),
                TxFillable::Gas { gas } => builder.set_gas_limit(gas),
            }
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
        let input = TransactionInput::new(tx.input().unwrap_or_default().clone());
        let rpc_tx = TransactionRequest {
            from: tx.from(),
            to: Some(tx.to().into()),
            value: tx.value(),
            chain_id: tx.chain_id(),
            input,
            nonce: tx.nonce(),
            max_fee_per_gas: tx.max_fee_per_gas(),
            max_priority_fee_per_gas: tx.max_priority_fee_per_gas(),
            gas: tx.gas_limit(),
            access_list: tx.access_list().cloned(),
            gas_price: tx.gas_price(),
            ..Default::default()
        };

        let fillable = self.prefixed_tx_fillable(rpc_tx).await;

        Ok(fillable.unwrap())
    }
}

impl DeadbeefFiller {
    pub fn set_iteration_mode(&mut self, mode: IterationMode) {
        self.iteration_mode = mode;
    }

    pub async fn prefixed_tx(&self, tx: TransactionRequest) -> Result<TransactionRequest> {
        let mut src_tx = tx.clone();

        let fillable = self.prefixed_tx_fillable(tx).await?;

        match fillable {
            TxFillable::Value { value } => {
                src_tx.value = Some(value);
            }
            TxFillable::Gas { gas } => {
                src_tx.gas = Some(gas);
            }
        }

        Ok(src_tx)
    }

    async fn prefixed_tx_fillable(&self, tx: TransactionRequest) -> Result<TxFillable> {
        init_logs();
        let max_cores = available_parallelism().unwrap().get() as u128;
        let field = match self.iteration_mode {
            IterationMode::Value => "value",
            IterationMode::Gas => "gas",
        };
        info!(
            "Looking for '0x{}' tx hash prefix using {max_cores} CPU cores, iterating on '{field}'",
            self.prefix
        );
        let max_value = max_iterations_for_prefix(self.prefix.len() as u32) * 2; // multiply by 2 to avoid searach space overlap in case 99% certainty fails
        let mut handles = vec![];
        let (done, _) = broadcast::channel(1);
        for i in 0..max_cores {
            let tx = tx.clone();
            let prefix = self.prefix.clone();
            let wallet = self.wallet.clone();
            let mut done = done.subscribe();

            let max_iters = max_value / max_cores;
            let iteration_mode = self.iteration_mode.clone();

            let handle = tokio::spawn(async move {
                match iteration_mode {
                    IterationMode::Value => {
                        let value = value_for_prefix(
                            tx,
                            wallet,
                            &mut done,
                            i * max_iters,
                            prefix,
                            max_cores,
                        )
                        .await;
                        match value {
                            Ok(Some(value)) => Some(TxFillable::Value { value }),
                            Err(e) => {
                                error!("Error: {:?}", e);
                                None
                            }
                            _ => None,
                        }
                    }
                    IterationMode::Gas => {
                        let gas = gas_for_prefix(
                            tx,
                            wallet,
                            &mut done,
                            (i * max_iters) as u64,
                            prefix,
                            max_cores,
                        )
                        .await;
                        match gas {
                            Ok(Some(gas)) => Some(TxFillable::Gas { gas }),
                            Err(e) => {
                                error!("Error: {:?}", e);
                                None
                            }
                            _ => None,
                        }
                    }
                }
            });
            handles.push(handle);
        }

        let (fillable, _index, _remaining) = select_all(handles).await;

        let _ = done.send(());
        Ok(fillable.unwrap().unwrap())
    }
}

fn init_logs() {
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .finish();
    _ = set_global_default(subscriber);
}

async fn value_for_prefix(
    tx: TransactionRequest,
    wallet: EthereumWallet,
    done: &mut tokio::sync::broadcast::Receiver<()>,
    starting_input: u128,
    prefix: String,
    max_cores: u128,
) -> Result<Option<U256>> {
    let mut value = starting_input;
    let mut buf = Vec::with_capacity(200);

    let result: Option<U256> = loop {
        select! {
            biased;
            _ = done.recv() => {
                break None;
            }
            _ = future::ready(()) => {
                let tx = tx.clone();
                let next_value = tx.value.unwrap_or_default() + U256::from(value);
                let tx_hash = tx_hash_for_value(tx, &wallet, next_value, &mut buf).await?;
                value += 1;

                let hash_str = format!("{:x}", &tx_hash);
                if hash_str.starts_with(&prefix) {
                    let iters = value - starting_input;
                    let total_iters = max_cores * iters;
                    info!("Found matching tx hash: {tx_hash} after ~{total_iters} iterations");
                    break Some(next_value);
                }
            }
        }
    };

    Ok(result)
}

async fn gas_for_prefix(
    tx: TransactionRequest,
    wallet: EthereumWallet,
    done: &mut tokio::sync::broadcast::Receiver<()>,
    starting_input: u64,
    prefix: String,
    max_cores: u128,
) -> Result<Option<u64>> {
    let mut value = starting_input;
    let mut buf = Vec::with_capacity(200);

    let result: Option<u64> = loop {
        select! {
            biased;
            _ = done.recv() => {
                break None;
            }
            _ = futures::future::ready(()) => {
                let tx = tx.clone();
                let next_value = tx.gas.unwrap_or_default() + value;
                let tx_hash = tx_hash_for_gas(tx, &wallet, next_value, &mut buf).await?;

                let hash_str = format!("{:x}", &tx_hash);

                value += 1;

                if hash_str.starts_with(&prefix) {
                    let iters = (value - starting_input) as u128;
                    let total_iters = max_cores * iters;
                    info!("Found matching tx hash: {tx_hash} after ~{total_iters} iterations");
                    break Some(next_value);
                }
            }
        }
    };

    Ok(result)
}

async fn tx_hash_for_value(
    tx: TransactionRequest,
    wallet: &EthereumWallet,
    value: U256,
    buf: &mut Vec<u8>,
) -> Result<FixedBytes<32>> {
    let mut tx = tx;
    buf.clear();
    tx.value = Some(value);
    let tx_envelope = tx.build(&wallet).await?;
    tx_envelope.encode_2718(buf);
    let tx_hash = keccak256(&buf);
    Ok(tx_hash)
}

async fn tx_hash_for_gas(
    tx: TransactionRequest,
    wallet: &EthereumWallet,
    gas: u64,
    buf: &mut Vec<u8>,
) -> Result<FixedBytes<32>> {
    let mut tx = tx;
    buf.clear();
    tx.gas = Some(gas);
    let tx_envelope = tx.build(&wallet).await?;
    tx_envelope.encode_2718(buf);
    let tx_hash = keccak256(&buf);
    Ok(tx_hash)
}

fn max_iterations_for_prefix(prefix_length: u32) -> u128 {
    let q = 1.0 / 16f64.powi(prefix_length as i32);
    let max_iterations = (4.605 / q).ceil();
    max_iterations as u128
}

#[cfg(test)]

mod tests {
    use super::*;
    use alloy::{
        network::EthereumWallet,
        node_bindings::Anvil,
        primitives::{Bytes, U256},
        providers::{Provider, ProviderBuilder},
        rpc::types::TransactionRequest,
        signers::local::PrivateKeySigner,
    };

    const TX_PREFIX: &str = "de";

    #[tokio::test(flavor = "multi_thread")]
    async fn test_prefixed_tx_by_value() -> Result<()> {
        let anvil = Anvil::new().spawn();
        let account = anvil.addresses()[0];
        let private_key = anvil.keys()[0].clone();
        let wallet = EthereumWallet::from(PrivateKeySigner::from(private_key));

        let mut deadbeef = DeadbeefFiller::new(TX_PREFIX.to_string(), wallet.clone()).unwrap();
        deadbeef.set_iteration_mode(IterationMode::Value);

        let anvil_provider = ProviderBuilder::new()
            .filler(deadbeef)
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

        let tx_hash = res.transaction_hash;
        let tx_hash = format!("{:x}", &tx_hash);
        let tx_prefix = &tx_hash[..TX_PREFIX.len()];

        assert_eq!(tx_prefix.as_bytes(), TX_PREFIX.as_bytes());

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_prefixed_tx_by_gas() -> Result<()> {
        let anvil = Anvil::new().spawn();
        let account = anvil.addresses()[0];
        let private_key = anvil.keys()[0].clone();
        let wallet = EthereumWallet::from(PrivateKeySigner::from(private_key));

        let mut deadbeef = DeadbeefFiller::new(TX_PREFIX.to_string(), wallet.clone()).unwrap();
        deadbeef.set_iteration_mode(IterationMode::Gas);

        let anvil_provider = ProviderBuilder::new()
            .fetch_chain_id()
            .filler(deadbeef)
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
            input: TransactionInput::new(Bytes::from("hellothere")),
            nonce: Some(nonce),
            gas_price: Some(gas_price * 110 / 100),
            gas: Some(210000),
            ..Default::default()
        };

        let res = anvil_provider
            .send_transaction(tx)
            .await?
            .get_receipt()
            .await?;

        let tx_hash = res.transaction_hash;
        let tx_hash = format!("{:x}", &tx_hash);
        let tx_prefix = &tx_hash[..TX_PREFIX.len()];

        assert_eq!(tx_prefix.as_bytes(), TX_PREFIX.as_bytes());

        Ok(())
    }
}
