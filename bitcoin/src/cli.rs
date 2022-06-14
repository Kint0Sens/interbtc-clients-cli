#![cfg(feature = "cli")]

use crate::{BitcoinCore, BitcoinCoreBuilder, Error};
use bitcoincore_rpc::{bitcoin::Network, Auth};
use clap::Parser;
use std::time::Duration;

#[derive(Parser, Debug, Clone)]
pub struct BitcoinOpts {
    #[clap(long, env = "BITCOIN_RPC_URL")]
    pub bitcoin_rpc_url: String,

    #[clap(long, env = "BITCOIN_RPC_USER")]
    pub bitcoin_rpc_user: String,

    #[clap(long, env = "BITCOIN_RPC_PASS")]
    pub bitcoin_rpc_pass: String,

    /// Timeout in milliseconds to wait for connection to bitcoin-core.
    #[clap(long, default_value = "60000")]
    pub bitcoin_connection_timeout_ms: u64,

    // /// Path to the json file containing bitcoin rpc user and pass.
    // /// Valid content of this file is e.g.
    // /// `{ "keyname": ["rpcuser", "rpcpassword"],  "keyname2": ["rpcuser2", "rpcpassword2"] }`.
    // #[clap(long, requires = "btc_keyname", default_value = "./btc_keyfile.json")]  // "~/keyfile.json" does not translate to /home/user/keyfile.json
    // pub btc_keyfile: String,

    // /// The name of the btc descriptors from the keyfile_btc to use.
    // #[clap(long, requires = "keyfile_btc", default_value = "keyname_btc")]
    // pub btc_keyname: String,


    /// Url of the electrs server - used for theft reporting. If unset, a default
    /// fallback is used depending on the detected network.
    #[clap(long)]
    pub electrs_url: Option<String>,
}

impl BitcoinOpts {
    fn new_auth(&self) -> Auth {
        Auth::UserPass(self.bitcoin_rpc_user.clone(), self.bitcoin_rpc_pass.clone())
    }

    fn new_client_builder(&self, wallet_name: Option<String>) -> BitcoinCoreBuilder {
        BitcoinCoreBuilder::new(self.bitcoin_rpc_url.clone())
            .set_auth(self.new_auth())
            .set_wallet_name(wallet_name)
            .set_electrs_url(self.electrs_url.clone())
    }

    pub async fn new_client(&self, wallet_name: Option<String>) -> Result<BitcoinCore, Error> {
        self.new_client_builder(wallet_name)
            .build_and_connect(Duration::from_millis(self.bitcoin_connection_timeout_ms))
            .await
    }

    pub fn new_client_with_network(&self, wallet_name: Option<String>, network: Network) -> Result<BitcoinCore, Error> {
        self.new_client_builder(wallet_name).build_with_network(network)
    }
    // pub fn get_btc_keys(&self) -> Result<(String,String, String),Error> {
    //     // load btc credentials
    //     let (rpcuser,rpcpassword) =
    //         get_btc_credentials_from_file(self.btc_keyfile.as_ref(), self.btc_keyname.as_ref())?;

    //     Ok((rpcuser,rpcpassword,self.btc_keyname.clone()))
    // }
}    
// fn get_btc_credentials_from_file(file_path: &str, keyname_btc: &str) -> Result<(String, String), KeyLoadingError> {
//     let file = std::fs::File::open(file_path)?;
//     let reader = std::io::BufReader::new(file);
//     let map : HashMap<String, Vec<String>> = serde_json::from_reader(reader)?;
//     let desc_pair = map.get(keyname_btc).ok_or(KeyLoadingError::KeyNotFound)?;
//     Ok((desc_pair[0].clone(),desc_pair[1].clone()))
// }