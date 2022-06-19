# InterBTC Command Line Interface Client Tools

The crates below this folder contain a series of CLI tools to interact with your Kintsugi / Interlay vault or with the InterBtc parachain

Available commands:
- **explore** - List Vaults, check then Liquidation Vault
- **redeem** - Execute a Redeem
- **transfer** - Transfer KSM to the Relay Chain (Kusama / Polkadot)
- **burn** - Burn wrapped tokens (kBTC/iBTC)
- **issue_request** - Trigger an issue request on a vault
- **issue_pay** - Pay an issue request from a BTC wallet
- **issue** - Combines issue_request and issue pay


## Todos

## Responsibilities

## Getting Started

Run the interBTC burn client:

```
cargo run
```

### Options

When using cargo to run this binary, arguments to cargo and the binary are separated by `--`. For example, to pass `--help` to the faucet to get a list of all command line options that is guaranteed to be up date, run:

```
cargo run -- --help
```

For convenience, a copy of this output is included below.
```
```

Example command when run on Kintsugi testnet
./target/release/burn  \
 --amount 2000

./target/release/burn  \
 --keyfile ~/.mytestvault/keyfile.json \
 --keyname interlaymaincustomeraccount \
 --amount 2000

 