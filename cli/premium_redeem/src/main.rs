mod error;

use clap::Parser;
use git_version::git_version;

//Tool code
use error::Error;

//interBTC related
use runtime::{
        H160,   
        substrate_subxt::PairSigner, 
        pallets::redeem::RequestRedeemEvent,
        RedeemPallet,
        VaultRegistryPallet,
        InterBtcRuntime,
        BtcAddress,
        AccountId
    };
use bitcoin::PartialAddress;

const VERSION: &str = git_version!(args = ["--tags"]);
const AUTHORS: &str = env!("CARGO_PKG_AUTHORS");
const NAME: &str = env!("CARGO_PKG_NAME");
const ABOUT: &str = env!("CARGO_PKG_DESCRIPTION");

#[derive(Parser)]
#[clap(name = NAME, version = VERSION, author = AUTHORS, about = ABOUT)]
struct Cli {
    /// Keyring / keyfile options containng the user's info
    #[clap(flatten)]
    account_info: runtime::cli::ProviderUserOpts,

    /// Connection settings for the BTC Parachain.
    #[clap(flatten)]
    connection_options: runtime::cli::ConnectionOpts,

 
    /// Settings specific to the cli tool.
    #[clap(flatten)]
    config: ToolConfig,
}

#[derive(Parser, Clone)]
pub struct ToolConfig {
    /// Amount to redeem, in satoshis
    #[clap(long, default_value = "0")]
    amount: u128,

    // /// Beneficiary Btc Wallet address. In string format
    #[clap(long, default_value = "*")]
    btc_address: String,

    /// Vault to redeem from
    #[clap(long, default_value = "")]
    vault_id: AccountId,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let cli: Cli = Cli::parse();
    env_logger::init_from_env(init_logger(cli.verbose));


    let config = opts.config;

    let amount = config.amount;
    let btc_address : BtcAddress = BtcAddress::decode_str(&config.btc_address).unwrap();
    let vault_id = config.vault_id;

    // User keys
    let (key_pair, _) = opts.account_info.get_key_pair()?;
    let signer = PairSigner::<InterBtcRuntime, _>::new(key_pair);
    
   
    // Connect to the parachain with the user keys
    let parachain_connection_options = opts.connection_options  ;
    let parachain = parachain_connection_options.try_connect(signer.clone()).await?;
    tracing::info!("Connected.");

    // Get list of premium redeem Vaults
     let result = parachain.get_premium_redeem_vaults().await;
    //  let result = parachain.get_vaults_with_issuable_tokens().await?;
     
     tracing::info!("Call done.");
     match  result {
         Ok(premium_redeem_vaults) => tracing::info!("{:#?}",premium_redeem_vaults[0]),
         Err(error) => tracing::info!("Error returned: {:?}",error),
     };


// Add this to runtime/src/rpc.rs or create "myrpc"
// async fn get_premium_redeem_vaults(&self) -> Result<Vec<AccountId>, Error>;

//    /// Fetch all vaults in premium redeem state.
//    async fn get_premium_redeem_vaults(&self) -> Result<Vec<AccountId>, Error> {
//     let head = self.get_latest_block_hash().await?;
//     let call_result: Vec<(AccountId, BalanceWrapper<u128>)> = self
//     .rpc_client
//     .request(
//         "vaultRegistry_getPremiumRedeemVaults",
//         &[to_json_value(head)?],
//     )
//     .await?;

//     Ok(call_result.into_iter().map(|(account_id, _)| account_id).collect())
}         
   
    Ok(())
     
    }


