# Redeem Client

Issue pay command line tool.

## Responsibilities

This tool will pay all pending requests for the associated account.

## Getting Started

### Build
cargo build --release --features parachain-metadata-testnet
 (testnet2022)

### Run
Run the interBTC issue pay cli client:

--vault-account-id points to the vault where the issue should occur

./target/release/issue  \
--keyfile ~/.mytestvault/keyfile.json  \
--keyname interlaymaincustomeraccount  \
--btc-parachain-url 'wss://api-testnet.interlay.io:443/parachain' \
--vault-account-id 5EqTVHyXde3pEck9cWKnuUkrDLtkHn3Bg6zRTHQEik1pYMbv 

./target/debug/issue  \
--keyfile ~/.mytestvault/keyfile.json  \
--keyname interlaymaincustomeraccount  \
--btc-parachain-url 'wss://api-testnet.interlay.io:443/parachain' \
--vault-account-id 5EqTVHyXde3pEck9cWKnuUkrDLtkHn3Bg6zRTHQEik1pYMbv 
```

### Options

When using cargo to run this binary, arguments to cargo and the binary are separated by `--`. For example, to pass `--help` to the faucet to get a list of all command line options that is guaranteed to be up date, run:

```
cargo run -- --help
```

For convenience, a copy of this output is included below.
```
```

--simulation    List the pending issues and prepare the btc transaction. Do not sign it

