use alloy_deadbeef::prefixed_tx;
use criterion::{criterion_group, criterion_main, Criterion};

use alloy::{
    network::EthereumWallet,
    primitives::{address, Address, U256},
    rpc::types::TransactionRequest,
    signers::local::PrivateKeySigner,
};

use tokio::runtime::Builder;

static ME: Address = address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266");
static PK: &str = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

fn beef_benchmark(c: &mut Criterion) {
    let runtime = Builder::new_multi_thread().enable_all().build().unwrap();

    let signer: PrivateKeySigner = PK.parse().unwrap();
    let wallet = EthereumWallet::from(signer);

    let tx = TransactionRequest {
        from: Some(ME),
        to: Some(ME.into()),
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
            .iter(|| prefixed_tx(tx.clone(), wallet.clone(), "beef"))
    });
}

criterion_group!(benches, beef_benchmark);
criterion_main!(benches);
