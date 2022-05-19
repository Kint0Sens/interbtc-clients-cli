mod error;

use std::env;
use clap::Parser;

use git_version::git_version;
use std::str::FromStr;
use std::ops::RangeInclusive;
use common::*;
use std::thread;
use std::time::Duration;


//cli related
use error::Error;
//interBTC related
use runtime::{
        VaultRegistryPallet,
        RedeemPallet,
        CollateralBalancesPallet,
        InterBtcSigner,
        UtilFuncs,
        BtcAddress,
        Ss58Codec,
        VaultId,
        AccountId,

        parse_collateral_currency,
        parse_wrapped_currency,
        };
use bitcoin::PartialAddress;

const VERSION: &str = git_version!(args = ["--tags"]);
const AUTHORS: &str = env!("CARGO_PKG_AUTHORS");
const NAME: &str = env!("CARGO_PKG_NAME");
const ABOUT: &str = env!("CARGO_PKG_DESCRIPTION");
const TOO_FEW_SATS: RangeInclusive<u128> = 1..=1999;


#[derive(Parser)]
#[clap(name = NAME, version = VERSION, author = AUTHORS, about = ABOUT)]
struct Cli {
    /// Return all logs
     /// Overridden by RUST_LOG env variable
     #[clap(short, long, parse(from_occurrences))]
    verbose: usize,

    /// Keyring / keyfile options containng the user's info
    #[clap(flatten)]
    account_info: runtime::cli::ProviderUserOpts,

    /// Connection settings for the BTC Parachain.
    #[clap(flatten)]
    parachain: runtime::cli::ConnectionOpts,

     /// Settings specific to the cli tool.
    #[clap(flatten)]
    config: ToolConfig,
}

#[derive(Parser, Clone)]
pub struct ToolConfig {
    /// Amount to redeem, in satoshis, 
    /// must be greater than Bridge Fee + BTC Network Fee + BTC Dust Limit 
    #[clap(long, validator = amount_gt_minimal)]
    redeem_amount: u128,

    /// Minimum wallet amount of wrapped token in sat, 
    /// bot will not trigger redeem when balance is below this amount
    #[clap(long)]
    minimum_wrapped: u128,

    /// Sleep time before checking balance again
    ///  when not enough wrapped balance
    #[clap(long, default_value = "15")]
    sleeptime_not_enough_balance: u64,

    /// Sleep time before checking balance again
    /// when no premium redeem vault available
    #[clap(long, default_value = "60")]
    sleeptime_no_premium_vault: u64,
    // /// Beneficiary Btc Wallet address. In string format
    #[clap(long)]
    btc_address: String,

    /// Vault to redeem from - account
    #[clap(long)]
    vault_account_id: AccountId,

    /// Vault to redeem from - collateral
    #[clap(long, default_value = "KSM")] 
    vault_collateral_id: String,

    /// Vault to redeem from
    #[clap(long, default_value = "KBTC")] 
    vault_wrapped_id: String,
}
#[allow(unreachable_code)]
#[tokio::main]
async fn main() -> Result<(), Error> {
    let cli: Cli = Cli::parse();
    env_logger::init_from_env(init_logger(cli.verbose));
 
 
    let config = cli.config;
    let redeem_amount = config.redeem_amount;
    let btc_address : BtcAddress = BtcAddress::decode_str(&config.btc_address).unwrap();
    let collateral_id  = parse_collateral_currency(&config.vault_collateral_id).unwrap();
    let wrapped_id  = parse_wrapped_currency(&config.vault_wrapped_id).unwrap();
    let vault_id = VaultId::new(config.vault_account_id, collateral_id, wrapped_id);

    // User keys
    let (key_pair, _) = cli.account_info.get_key_pair()?;
    let signer = InterBtcSigner::new(key_pair);
    let signer_account_id = signer.account_id().clone();
  
    // Connect to the parachain with the user keys
    let (shutdown_tx, _) = tokio::sync::broadcast::channel(16);
    let parachain_config = cli.parachain;
    tracing::trace!("TEXT_CONNECT_ATTEMPT");
    let parachain = parachain_config.try_connect(signer.clone(), shutdown_tx.clone()).await?;
    tracing::info!("TEXT_CONNECTED");

    tracing::info!("Signer:         {}",signer_account_id.to_ss58check());
    tracing::info!("Vault:          {}",vault_id.account_id.to_ss58check());
    tracing::info!("BTC Address     {}",config.btc_address);
    tracing::info!("BTC Address     {:?}",btc_address);
    tracing::info!("Redeem amount:  {} {} Sat",config.redeem_amount, config.vault_wrapped_id);
 

    let signer_account_id = parachain.get_account_id();

    //Main loop
    loop {
        // Is there enough wrapped balance to proceed?
        let balance_wrapped = parachain.get_free_balance_for_id(signer_account_id.clone(),wrapped_id).await?;
        tracing::info!("{} balance:        {}  Sat", config.vault_wrapped_id,balance_wrapped);
     
        if balance_wrapped < config.minimum_wrapped {
            tracing::warn!("{} balance lower than minimum balance of {}  Sat", config.vault_wrapped_id, config.minimum_wrapped);
            tracing::info!("Waiting {} seconds before checking again", config.sleeptime_not_enough_balance);
            thread::sleep(Duration::from_secs(config.sleeptime_not_enough_balance));
            continue;
        }
        // Is there some premium redeem available on a vault
        let result = parachain.get_premium_redeem_vaults().await;
        match result {
            Ok(premium_vaults) => {
                if premium_vaults.len() == 0 {
                    tracing::warn!("No premium redeem vault found");
                    tracing::info!("Waiting {} seconds before checking again", config.sleeptime_no_premium_vault);
                    thread::sleep(Duration::from_secs(config.sleeptime_no_premium_vault));
                    continue;
                }
        
            }
            Err(error) => {
                tracing::error!("Error when checking for premium vaults");
                tracing::error!("{:?}",error);
                continue;
            }
        }

        let premium_vaults = result.unwrap();
        // select 1st vault with sufficient premium redeemable amount compared to mawimum_redeem
        // if none match the maximum_redeem get the greatest amt
        let mut max_premiumm_amt; 
        let mut index = 0;
        let mut vault_index : i32;
        for (vault, premium_amt) in premium_vaults.into_iter() {
            if premium_amt.amount > config.redeem_amount {
                // Found eligible vault. use it

            };
            if max_premiumm_amt <= premium_amt.amount {
                max_premiumm_amt = premium_amt.amount;
                vault_index = index;
            }; 
            index = index + 1;
        };
        // Redeem
        // Send redeem request
        // let _redeem_id = parachain.request_redeem(amount, btc_address, &vault_id).await?;
        // tracing::info!("Vault {} confirmed redeem request of {} {} Sat to BTC address {}",
        //         vault_id.account_id.to_ss58check(),
        //         amount,
        //         config.vault_wrapped_id,
        //         btc_address.encode_str(BITCOIN_NETWORK).unwrap());

        // Evaluate the reward


    }


 
    // if balance < amount {
    //     tracing::error!("Insufficient {} Balance - Cancelling", config.vault_wrapped_id);
    //     return Ok(())
    // }    

    
    Ok(())
     
    }

    fn amount_gt_minimal(s: &str) -> Result<(), String> {
        //TODO: Dynamic calc of minimal amount?
        u128::from_str(s)
        .map(|amt| !TOO_FEW_SATS.contains(&amt))
        .map_err(|e| e.to_string())
        .and_then(|result| match result {
            true => Ok(()),
            false => Err(format!(
                "Amount in Sat should exceed {}",
                TOO_FEW_SATS.end()
            )),
        })
    }
