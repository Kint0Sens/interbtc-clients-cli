use common::Error;
use std::env;
use clap::Parser;
use runtime::Error as RuntimeError;

use git_version::git_version;
use common::*;
use std::thread;
use std::time::Duration;
use module_oracle_rpc_runtime_api::BalanceWrapper;

use runtime::{
        VaultRegistryPallet,
        VaultId,
        VaultStatus,
        RedeemPallet,
        CollateralBalancesPallet,
        InterBtcSigner,
        UtilFuncs,
        BtcAddress,
        Ss58Codec,
        CurrencyIdExt,
        // CurrencyInfo,
        PrettyPrint,
        InterBtcParachain,
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

    /// For testing consider all active vaults 
    /// as premium vaults.
    /// Useful to test tool when no premium redeem vaults exist
    #[clap(long, parse(from_occurrences))]
    treat_all_vaults_as_premium: usize,

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
    #[clap(long, validator = amount_gt_minimal, default_value = "2100")]
    max_redeem_amount: u128,

    /// Minimum wallet balance amount of wrapped token in sat, 
    /// bot will not trigger redeem when balance is below this amount
    #[clap(long, default_value = "2000")]
    min_wrapped_balance: u128,

    /// Sleep time before checking balance again
    ///  when not enough wrapped balance
    #[clap(long, default_value = "15")]
    sleeptime_not_enough_balance: u64,

    /// Sleep time before checking balance again
    /// when no premium redeem vault available
    #[clap(long, default_value = "60")]
    sleeptime_no_premium_vault: u64,

    /// Sleep time after each succesful redeem loop
    #[clap(long, default_value = "10")]
    sleeptime_main_loop: u64,

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
    tracing::trace!("{}",TEXT_CONNECT_ATTEMPT);
    let parachain = parachain_config.try_connect(signer.clone(), shutdown_tx.clone()).await?;
    tracing::info!("{}",TEXT_CONNECTED);
    tracing::info!("{}",TEXT_SEPARATOR);
 
    
    let native_id = parachain.get_native_currency_id();
    // let signer_account_id = parachain.get_account_id();
    let mut balance_wrapped = parachain.get_free_balance_for_id(signer_account_id.clone(),wrapped_id).await?;
    let mut balance_collateral = parachain.get_free_balance_for_id(signer_account_id.clone(),collateral_id).await?;
    let mut balance_native = parachain.get_free_balance_for_id(signer_account_id.clone(),native_id).await?;


    tracing::info!("Signer:                     {}",signer_account_id.to_ss58check());
    // tracing::info!("Vault:                 {}",vault_id.account_id.to_ss58check());
    tracing::info!("Redeem BTC address:         {}",config.btc_address);
    // tracing::info!("BTC Address:           {:?}",btc_address);
    tracing::info!("Max redeem amount:          {} {} sat",config.max_redeem_amount, config.chain_wrapped_id);
    tracing::info!("Min wrapped balance:        {} {} sat",config.min_wrapped_balance, config.chain_wrapped_id);
    tracing::info!("Initial wrapped balance:    {} {} sat", balance_wrapped, config.chain_wrapped_id);
    tracing::info!("Initial collateral balance: {} {} planck", balance_collateral, config.chain_collateral_id);
    tracing::info!("Initial native balance:     {} {} planck", balance_native, get_currency_str(native_id.inner()));

    //Main loop
    // Check available wrapped balance
    // Identify Premium Redeem Vault
    // Request Redeem
    // Report KSM Gain
    // repeat

    let mut loop_iteration : i32= 0;
    loop {
        loop_iteration = loop_iteration + 1;
        tracing::info!("[{}]{}",loop_iteration,TEXT_SEPARATOR);
        // Is there enough wrapped balance to proceed?
        if balance_wrapped < config.min_wrapped_balance {
            tracing::warn!("{} balance lower than minimum balance of {}  Sat", config.chain_wrapped_id, config.min_wrapped_balance);
            tracing::info!("Waiting {} seconds before checking again", config.sleeptime_not_enough_balance);
            thread::sleep(Duration::from_secs(config.sleeptime_not_enough_balance));
            continue;
        } else {
            tracing::info!("Sufficient {} balance to attempt premium redeems", config.chain_wrapped_id);
        };

        // Are there some vaults with premium redeem available?
        // let result = parachain.get_premium_redeem_vaults().await;
        let result = get_premium_redeem_vaults_or_all_active(parachain.clone(), cli.treat_all_vaults_as_premium).await;
        match &result {
            Ok(premium_vaults) => {
                if premium_vaults.len() == 0 { // This should not occur. RPC returns error instead
                    tracing::warn!("No premium redeem vault found");
                    tracing::info!("Waiting {} seconds before checking again", config.sleeptime_no_premium_vault);
                    thread::sleep(Duration::from_secs(config.sleeptime_no_premium_vault));
                    continue;
                }
        
            }
            Err(error) => {
                let error_str = format!("{:?}",error); 
                if error_str.contains("Unable to find a vault below") {
                    tracing::warn!("No premium redeem vault found");
                    tracing::info!("Waiting {} seconds before checking again", config.sleeptime_no_premium_vault);
                    thread::sleep(Duration::from_secs(config.sleeptime_no_premium_vault));
                    continue;  
                } else {
                    match error {
                        RuntimeError::VaultNotFound => {
                            tracing::warn!("No redeem vault found");
                            tracing::info!("Waiting {} seconds before checking again", config.sleeptime_no_premium_vault);
                            thread::sleep(Duration::from_secs(config.sleeptime_no_premium_vault));
                            continue;  
                        },
                        _ => {
                            tracing::error!("Error when checking for premium vaults");
                            tracing::error!("{:?}",error);
                            tracing::info!("Waiting {} seconds before checking again", config.sleeptime_no_premium_vault);
                            thread::sleep(Duration::from_secs(config.sleeptime_no_premium_vault));
                            continue;
                        }
                    }
                }
            }
        }

        let premium_vaults = result.unwrap();
        // select 1st vault with sufficient premium redeemable amount compared to configured max_redeem_amount
        // if none match the max_redeem_amount get the greatest amt
        let mut max_premium_amt = 0; 
        let mut index : usize = 0;
        let mut vault_index : usize = 0;

        for (_, loop_premium_amt) in premium_vaults.iter() {
            if loop_premium_amt.amount > config.max_redeem_amount {
                // Found eligible vault. use it
                // tracing::info!("Found. Index/Loop Amt/Vault_Index/max_premium_amt {}/{}/{}/{}",
                //       index,loop_premium_amt.amount,vault_index, max_premium_amt);
                vault_index = index;
                break;
            };
            if max_premium_amt <= loop_premium_amt.amount {
                max_premium_amt = loop_premium_amt.amount;
                vault_index = index;
                // tracing::info!("Search. Index/Loop Amt/Vault_Index/max_premium_amt {}/{}/{}/{}",
                //         index,loop_premium_amt.amount,vault_index, max_premium_amt);

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
        tracing::info!("Redeem request amount  {} {} Sat",
            redeem_amount,
            config.chain_wrapped_id);
        tracing::info!("Found vault {} with capacity {}", target_vault_id.account_id.pretty_print(), premium_amt.amount);

        tracing::info!("Sending redeem request to parachain");
        let _redeem_id = parachain.request_redeem(redeem_amount, btc_address, &target_vault_id).await?;
        tracing::info!("Parachain confirms redeem request to vault {} of {} {} sat to BTC address {}",
                target_vault_id.account_id.to_ss58check(),
                redeem_amount,
                config.chain_wrapped_id,
                btc_address.encode_str(BITCOIN_NETWORK).unwrap()
            );

        // Evaluate the reward by checking balances and reporting deltas
        let balance_wrapped_new = parachain.get_free_balance_for_id(signer_account_id.clone(),wrapped_id).await?;
        let balance_collateral_new = parachain.get_free_balance_for_id(signer_account_id.clone(),collateral_id).await?;
        let balance_native_new = parachain.get_free_balance_for_id(signer_account_id.clone(),native_id).await?;
        let delta_wrapped : i128 = balance_wrapped_new as i128 - balance_wrapped as i128;
        let delta_collateral : i128 = balance_collateral_new as i128 - balance_collateral as i128;
        let delta_native : i128 = balance_native_new as i128 - balance_native as i128;
        tracing::info!("Wrapped balance:          {} {} sat", balance_wrapped_new, config.chain_wrapped_id);
        tracing::info!("Collateral balance:       {} {} planck", balance_collateral_new, config.chain_collateral_id);
        tracing::info!("Native balance:           {} {} planck", balance_native_new, get_currency_str(native_id.inner()));
        tracing::info!("Delta wrapped balance:    {} {} sat", delta_wrapped, config.chain_wrapped_id);
        tracing::info!("Delta collateral balance: {} {} planck", delta_collateral, config.chain_collateral_id);
        tracing::info!("Delta native balance:     {} {} planck", delta_native, get_currency_str(native_id.inner()));
        balance_wrapped = balance_wrapped_new;
        balance_collateral = balance_collateral_new;
        balance_native = balance_native_new;

        tracing::info!("Waiting {} seconds before next loop iteration", config.sleeptime_main_loop);
        thread::sleep(Duration::from_secs(config.sleeptime_main_loop));

    }
    Ok(())  
}


 async fn get_premium_redeem_vaults_or_all_active(parachain: InterBtcParachain, treat_all_as_premium : usize) -> Result<Vec<(VaultId,BalanceWrapper<u128>)>,runtime::Error> {
    if treat_all_as_premium == 0 {
        parachain.get_premium_redeem_vaults().await
    } else {
        let vaults = parachain.get_all_vaults().await;
        let mut result : Vec<(VaultId,BalanceWrapper<u128>)> = Vec::new();
        match vaults {
            Ok(vaults) => {
                for vault in vaults.iter() {
                    match vault.status {
                        VaultStatus::Active(active) => { 
                            if active == true {
                                let redeemable = vault.issued_tokens - vault.to_be_redeemed_tokens;
                                result.push((vault.id.clone(), BalanceWrapper { amount: redeemable }))
                            }
                        },
                        _ => {}
                    };
                };
            }
            _  => {
                // Generate an error treated as no premium redeem vault
                return Err(RuntimeError::VaultNotFound);
            }
        }

        Ok(result)
    }
}