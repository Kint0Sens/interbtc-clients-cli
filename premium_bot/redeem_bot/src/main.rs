use std::env;
use clap::Parser;

use git_version::git_version;
use common::*;
use std::thread;
use std::time::Duration;

use runtime::{
        VaultRegistryPallet,
        RedeemPallet,
        CollateralBalancesPallet,
        InterBtcSigner,
        UtilFuncs,
        BtcAddress,
        Ss58Codec,
        CurrencyIdExt,
        CurrencyInfo,
        parse_collateral_currency,
        parse_wrapped_currency,
        };
use bitcoin::PartialAddress;

const VERSION: &str = git_version!(args = ["--tags"]);
const AUTHORS: &str = env!("CARGO_PKG_AUTHORS");
const NAME: &str = env!("CARGO_PKG_NAME");
const ABOUT: &str = env!("CARGO_PKG_DESCRIPTION");


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
    #[clap(long, validator = amount_gt_minimal, default_value = "999999999999999999999")]
    max_redeem_amount: u128,

    /// Minimum wallet amount of wrapped token in sat, 
    /// bot will not trigger redeem when balance is below this amount
    #[clap(long, default_value = "2000")]
    min_wrapped: u128,

    /// Sleep time before checking balance again
    ///  when not enough wrapped balance
    #[clap(long, default_value = "15")]
    sleeptime_not_enough_balance: u64,

    /// Sleep time before checking balance again
    /// when no premium redeem vault available
    #[clap(long, default_value = "60")]
    sleeptime_no_premium_vault: u64,

    /// Beneficiary Btc Wallet address. In string format
    #[clap(long)]
    btc_address: String,

    /// Collateral
    #[clap(long, default_value = "KSM")]  // Make network dependent default
    chain_collateral_id: String,

    /// Wrapped
    #[clap(long, default_value = "KBTC")] // Make network dependent default
    chain_wrapped_id: String,
}
#[allow(unreachable_code)]
#[tokio::main]
async fn main() -> Result<(), Error> {
    let cli: Cli = Cli::parse();
    env_logger::init_from_env(init_logger(cli.verbose));
 
 
    let config = cli.config;
    // let redeem_amount = config.redeem_amount;
    let btc_address : BtcAddress = BtcAddress::decode_str(&config.btc_address).unwrap();
    let collateral_id  = parse_collateral_currency(&config.chain_collateral_id).unwrap();
    let wrapped_id  = parse_wrapped_currency(&config.chain_wrapped_id).unwrap();
    // let vault_id = VaultId::new(config.vault_account_id, collateral_id, wrapped_id);

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
    let native_id = parachain.get_native_currency_id();
    // let signer_account_id = parachain.get_account_id();
    let mut balance_wrapped = parachain.get_free_balance_for_id(signer_account_id.clone(),wrapped_id).await?;
    let mut balance_collateral = parachain.get_free_balance_for_id(signer_account_id.clone(),collateral_id).await?;
    let mut balance_native = parachain.get_free_balance_for_id(signer_account_id.clone(),native_id).await?;


    tracing::info!("Signer:                {}",signer_account_id.to_ss58check());
    // tracing::info!("Vault:                 {}",vault_id.account_id.to_ss58check());
    tracing::info!("BTC Address:           {}",config.btc_address);
    tracing::info!("BTC Address:           {:?}",btc_address);
    tracing::info!("Max Redeem amount:     {} {} Sat",config.max_redeem_amount, config.chain_wrapped_id);
    tracing::info!("Min Wrapped balance:   {} {} Sat",config.max_redeem_amount, config.chain_wrapped_id);

    tracing::info!("Balances(sat/planck):  {}/{}/{} {}/{}/{:?}", 
        balance_wrapped,
        balance_collateral,
        balance_native,
        config.chain_wrapped_id,
        config.chain_collateral_id,
        native_id
    );
    tracing::info!("{}", native_id.inner().name().to_lowercase());

    //Main loop
    // Check available wrapped balance
    // Identify Premium Redeem Vault
    // Request Redeem
    // Report KSM Gain
    // repeat

    loop {
        // Is there enough wrapped balance to proceed?
        if balance_wrapped < config.min_wrapped {
            tracing::warn!("{} balance lower than minimum balance of {}  Sat", config.chain_wrapped_id, config.min_wrapped);
            tracing::info!("Waiting {} seconds before checking again", config.sleeptime_not_enough_balance);
            thread::sleep(Duration::from_secs(config.sleeptime_not_enough_balance));
            continue;
        }
        // Is there some premium redeem available on a vault
        let result = parachain.get_premium_redeem_vaults().await;
        match &result {
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
        // select 1st vault with sufficient premium redeemable amount compared to configured max_redeem_amount
        // if none match the max_redeem_amount get the greatest amt
        let mut max_premiumm_amt = 0; 
        let mut index : usize = 0;
        let mut vault_index : usize = 0;

        for (_, loop_premium_amt) in premium_vaults.iter() {
            if loop_premium_amt.amount > config.max_redeem_amount {
                // Found eligible vault. use it
                vault_index = index;
                break;
            };
            if max_premiumm_amt <= loop_premium_amt.amount {
                max_premiumm_amt = loop_premium_amt.amount;
                vault_index = index;
            }; 
            index = index + 1;
        };

        let (target_vault_id, premium_amt) =  &premium_vaults[vault_index];
        // Send redeem request
        let redeem_amount = if premium_amt.amount > config.max_redeem_amount {
            config.max_redeem_amount
        } else {
            premium_amt.amount
        };
        let _redeem_id = parachain.request_redeem(redeem_amount, btc_address, &target_vault_id).await?;
        tracing::info!("Parachain confirms redeem request to vault {} of {} {} Sat to BTC address {}",
                target_vault_id.account_id.to_ss58check(),
                redeem_amount,
                config.chain_wrapped_id,
                btc_address.encode_str(BITCOIN_NETWORK).unwrap()
            );

        // Evaluate the reward by checking balances and reporting deltas
        let balance_wrapped_new = parachain.get_free_balance_for_id(signer_account_id.clone(),wrapped_id).await?;
        let balance_collateral_new = parachain.get_free_balance_for_id(signer_account_id.clone(),collateral_id).await?;
        let balance_native_new = parachain.get_free_balance_for_id(signer_account_id.clone(),native_id).await?;
        tracing::info!("Balances(sat/planck):  {}/{}/{} {}/{}/{:?}", 
            balance_wrapped_new,
            balance_collateral_new,
            balance_native_new,
            config.chain_wrapped_id,
            config.chain_collateral_id,
            native_id
        );
        tracing::info!("{}", native_id.inner().name().to_lowercase());
        tracing::info!("Deltas(sat/planck):  {}/{}/{} {}/{}/{:?}", 
            balance_wrapped_new - balance_wrapped,
            balance_collateral_new - balance_collateral,
            balance_native_new - balance_native,
            config.chain_wrapped_id,
            config.chain_collateral_id,
            native_id
        );
        balance_wrapped = balance_wrapped_new;
        balance_collateral = balance_collateral_new;
        balance_native = balance_native_new;

    }
    Ok(())  
}

