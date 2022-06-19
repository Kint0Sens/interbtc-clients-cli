# Burn Client

Burn  command line client tool.

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

 