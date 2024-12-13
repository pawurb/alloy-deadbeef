use alloy_deadbeef::prefixed_tx_value;
use criterion::{criterion_group, criterion_main, Criterion};

use alloy::{
    network::EthereumWallet, node_bindings::Anvil, primitives::U256,
    rpc::types::TransactionRequest, signers::local::PrivateKeySigner,
};

use tokio::runtime::Builder;

fn beef_benchmark(c: &mut Criterion) {
    let anvil = Anvil::new().spawn();
    let runtime = Builder::new_multi_thread().enable_all().build().unwrap();
    let account = anvil.addresses()[0];
    let private_key = anvil.keys()[0].clone();
    let wallet = EthereumWallet::from(PrivateKeySigner::from(private_key));

    let tx = TransactionRequest {
        from: Some(account),
        to: Some(account.into()),
        value: Some(U256::ZERO),
        nonce: Some(123),
        chain_id: Some(1),
        max_fee_per_gas: Some(120),
        max_priority_fee_per_gas: Some(1),
        gas: Some(210000),
        ..Default::default()
    };

    c.bench_function("Find '0xbeef'", |b| {
        b.to_async(&runtime)
            .iter(|| prefixed_tx_value(tx.clone(), wallet.clone(), "beef"))
    });
}

criterion_group!(benches, beef_benchmark);
criterion_main!(benches);
