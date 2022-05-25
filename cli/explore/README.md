# interBTC Exlpore Client

Explore interBTC command line tool.

## Responsibilities

## Getting Started

Run the interBTC explore client:

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

./target/release/explore \
--keyfile ~/.mytestvault/keyfile.json \
--keyname interlaymaincustomeraccount \
vaults

./target/release/explore \
--keyfile ~/.mytestvault/keyfile.json \
--keyname interlaymaincustomeraccount \
liquidation-vault

./target/release/explore \
--keyfile ~/.mytestvault/keyfile.json \
--keyname interlaymaincustomeraccount \
--verbose \
vaults
