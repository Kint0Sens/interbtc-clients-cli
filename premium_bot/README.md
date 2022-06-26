# Redeem Client

Redeem command line tool.


## Todos
* Determine dust dynammically istead of requiring amount to be 2000+ sat

## Responsibilities

## Getting Started

Run the interBTC redeem client:

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

Example command when run on Kintsugi testnet (Q12022)

Here the user account is sotred in file keyfile.json on the current directory and the name of the account in the keyfile.json is 'keyname'
./target/release/redeem  \
 --btc-parachain-url 'wss://api-testnet.interlay.io:443/parachain' \
 --vault-account-id 5EqTVHyXde3pEck9cWKnuUkrDLtkHn3Bg6zRTHQEik1pYMbv \
 --btc-address tb1qs9w0p3vja4y6h00jg6sjkwtwfut5sjks3z9nt5 \
 --amount 2000

Here the user account is sotred in file ~/.mytestvault/keyfile.json and the name of the account in the keyfile.json is 'interlaymaincustomeraccount'
./target/release/redeem  \
 --keyfile ~/.mytestvault/keyfile.json \
 --keyname interlaymaincustomeraccount \
 --btc-parachain-url 'wss://api-testnet.interlay.io:443/parachain' \
 --vault-account-id 5EqTVHyXde3pEck9cWKnuUkrDLtkHn3Bg6zRTHQEik1pYMbv \
 --btc-address tb1qs9w0p3vja4y6h00jg6sjkwtwfut5sjks3z9nt5 \
 --amount 2000

