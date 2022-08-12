use std::env;
use clap::Parser;

use git_version::git_version;
// use std::ops::RangeInclusive;
use common::*;


//interBTC related
use runtime::{
        RedeemPallet,
        CollateralBalancesPallet,
        InterBtcSigner,
        UtilFuncs,
        Ss58Codec,

        parse_collateral_currency,
        parse_wrapped_currency,
        };

const VERSION: &str = git_version!(args = ["--tags"]);
const AUTHORS: &str = env!("CARGO_PKG_AUTHORS");
const NAME: &str = env!("CARGO_PKG_NAME");
const ABOUT: &str = env!("CARGO_PKG_DESCRIPTION");
// const TOO_FEW_SATS: RangeInclusive<u128> = 1..=1999;

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
    /// Amount to burnt, in satoshis, 
    #[clap(long)]
    amount: u128,
    //    #[clap(long, validator = amount_gt_minimal)]  // Use validator for minimal amount?

    /// Vault to redeem from - collateral
    #[clap(long, default_value = "KSM")] 
    vault_collateral_id: String,

    /// Vault to redeem from
    #[clap(long, default_value = "KBTC")] 
    vault_wrapped_id: String,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let cli: Cli = Cli::parse();
    env_logger::init_from_env(init_logger(cli.verbose));
 
 
    let config = cli.config;

    let amount = config.amount;
    let collateral_id  = parse_collateral_currency(&config.vault_collateral_id).unwrap();
    let wrapped_id  = parse_wrapped_currency(&config.vault_wrapped_id).unwrap();

    // User keys
    let (key_pair, _) = cli.account_info.get_key_pair()?;
    let signer = InterBtcSigner::new(key_pair);
    let signer_account_id = signer.account_id().clone();
  
 
    // Connect to the parachain with the user keys
    let (shutdown_tx, _) = tokio::sync::broadcast::channel(16);
    let parachain_config = cli.parachain;
    tracing::trace!("{}",TEXT_CONNECT_ATTEMPT);
    let parachain = parachain_config.try_connect(signer.clone(), shutdown_tx.clone()).await?;
    tracing::info!("{}",TEXT_CONNECTED);

    tracing::info!("{}",TEXT_SEPARATOR);
    tracing::info!("Signer:             {}",signer_account_id.to_ss58check());
    tracing::info!("Burn amount:        {} {} sat",config.amount, config.vault_wrapped_id);
  
    let signer_account_id = parachain.get_account_id();
    let balance_wrapped = parachain.get_free_balance_for_id(signer_account_id.clone(),wrapped_id).await?;
    let balance_collateral = parachain.get_free_balance_for_id(signer_account_id.clone(),collateral_id).await?;
    tracing::info!("Wrapped balance:    {} {} sat",balance_wrapped, config.vault_wrapped_id);
    tracing::info!("Collateral balance: {} {} planck",balance_collateral, config.vault_collateral_id);

    if balance_wrapped < amount {
        tracing::error!("Insufficient {} balance - Cancelling", config.vault_wrapped_id);
    }    

    // Send burn request
    tracing::info!("Sending burn request to parachain");
    let _result = parachain.liquidation_redeem(collateral_id,wrapped_id,amount).await?;
    tracing::info!("Burn of {} {} sat into {} successful",
             amount,
             config.vault_wrapped_id,
             config.vault_collateral_id);

    // Check balances
    let balance_wrapped = parachain.get_free_balance_for_id(signer_account_id.clone(),wrapped_id).await?;
    let balance_collateral = parachain.get_free_balance_for_id(signer_account_id.clone(),collateral_id).await?;
    tracing::info!("Balance:            {} {} sat",balance_wrapped, config.vault_wrapped_id);
    tracing::info!("Balance:            {} {} planck",balance_collateral, config.vault_collateral_id);


    Ok(())
     
    }


