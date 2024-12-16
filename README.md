# alloy-deadbeef [![Latest Version](https://img.shields.io/crates/v/alloy-deadbeef.svg)](https://crates.io/crates/alloy-deadbeef) [![GH Actions](https://github.com/pawurb/alloy-deadbeef/actions/workflows/ci.yml/badge.svg)](https://github.com/pawurb/alloy-deadbeef/actions)

This crate allows to generate custom, vanity tx hash prefixes:

![Vanity tx](https://github.com/pawurb/alloy-deadbeef/raw/main/deadbeef-tx-etherscan.png)

It brute-forces the correct hash iterating on `gas_limit` or `value` fields until it finds a matching prefix. You can read more about the implementation [in this blog post](https://pawelurbanek.com/alloy-deadbeef-vanity).

## Usage

```
cargo add alloy-deadbeef
```

You can use it as an as an [alloy provider filler](https://alloy.rs/building-with-alloy/understanding-fillers.html):

```rust
let provider = ProviderBuilder::new()
    .filler(DeadbeefFiller::new(
       "beef".to_string(),
       wallet.clone(),
    ))
    .wallet(wallet)
    .on_http(endpoint().parse()?);
```

All the transactions sent from this provider will land with a `0xbeef` prefix. If you combine it with other fillers, it has to be the last one. Otherwise hash calculations will be invalid.

Alternatively, you can generate a tx object that, once submitted will have a matching hash prefix:

```rust
let tx = TransactionRequest {
    from: Some(account),
    to: Some(account.into()),
    value: Some(U256::ZERO),
    chain_id: Some(chain_id),
    nonce: Some(nonce),
    max_fee_per_gas: Some(gas_price * 110 / 100),
    max_priority_fee_per_gas: Some(GWEI_I),
    gas: Some(21000),
    ..Default::default()
};
let deadbeef = DeadbeefFiller::new("beef".to_string(), wallet.clone())?;
let prefixed_tx = deadbeef.prefixed_tx(tx.clone()).await?;

let provider = ProviderBuilder::new()
    .wallet(wallet)
    .on_http(endpoint().parse()?);

 let _ = provider
    .send_transaction(tx)
    .await?
    .get_receipt()
    .await?;
```

For prefixes up to 4 characters, it's possible to send `nonpayable` transactions because we're iterating on `gas_limit` instead of `value`. [See this example](https://github.com/pawurb/alloy-deadbeef/blob/main/examples/vanity_weth_approve.rs) for details on how to do it.

Alternatively, you can force a specific iteration mode like this:

```rust
let mut deadbeef = DeadbeefFiller::new("beef".to_string(), wallet)?;
deadbeef.set_iteration_mode(IterationMode::Value);
```

## Processing time

The table shows the worst-case scenario processing times on MBP M2 with 12 CPU cores. [Check out the blog post](https://pawelurbanek.com/alloy-deadbeef-vanity) for more details.

<table>
  <tr>
    <th>Prefix length</th>
    <th>99% certainty time</th>
  </tr>
  <tr>
    <td>1</td>
    <td>547.58µs</td>
  </tr>
  <tr>
    <td>2</td>
    <td>5.81ms</td>
  </tr>
  <tr>
    <td>3</td>
    <td>123.86ms</td>
  </tr>
  <tr>
    <td>4</td>
    <td>1.82s</td>
  </tr>
  <tr>
    <td>5</td>
    <td>29.62s</td>
  </tr>
  <tr>
    <td>6</td>
    <td>~8 minutes</td>
  </tr>
  <tr>
    <td>7</td>
    <td>~128 minutes</td>
  </tr>
  <tr>
    <td>8</td>
    <td>~34 hours</td>
  </tr>
  <tr>
    <td>9</td>
    <td>~23 days</td>
  </tr>
</table>

## Status

I'd appreciate feedback on how hash crunching speed can be improved. [There's a benchmark](https://github.com/pawurb/alloy-deadbeef/blob/main/benches/find_bee.rs) to easily measure changes in performance:

```bash
cargo bench
```
