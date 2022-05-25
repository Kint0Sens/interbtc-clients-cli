# XCM Transfer Client

Command line client tool to transfer KSMs from Interbtc to Kusama.

## Getting Started

Build the project with
cargo build --release  --features parachain-metadata-testnet  for an executable to run on the Kintsugi Testnet parachain
cargo build --release  --features parachain-metadata-kintsugi for an executable to run on the Kintsugi parachain
cargo build --release  --features parachain-metadata-interlay for an executable to run on the Interlay parachain


### Options

W``

Example command when run on Kintsugi testnet with default keyfile / keyname

./target/release/transfer  \
 --amount 2000

 