use common::Error;
use std::env;
use clap::Parser;
use runtime::Error as RuntimeError;

use git_version::git_version;
use common::*;
// use std::thread;
// use std::time::Duration;
use module_oracle_rpc_runtime_api::BalanceWrapper;

use runtime::{
    VaultRegistryPallet,
    VaultId,
    VaultStatus,
    RedeemPallet,
    CollateralBalancesPallet,
    InterBtcSigner,
    UtilFuncs,
    Ss58Codec,
    CurrencyIdExt,
    PrettyPrint,
    InterBtcParachain,
    BtcAddress,
    parse_collateral_currency,
    parse_wrapped_currency,
};
use bitcoin::{
    PartialAddress,
    BitcoinCoreApi,
    Amount,
};

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

    // /// Wait for bitcoin network confirmation of redeems
    // #[clap(long, parse(from_occurrences))]
    // wait_for_btc_confirmation: usize,

     /// Confirmations needed for bitcoin balance checks and transfer check
    /// If omitted, defaults to 1. If set to 0 transfer completion will not be checked
    /// but balance checks will still use a default of 1
    #[clap(short, long, default_value = "1" )]
    btc_network_confirmations: u32,

    /// Keyring / keyfile options containng the user's info
    #[clap(flatten)]
    account_info: runtime::cli::ProviderUserOpts,

    /// Connection settings for the BTC Parachain.
    #[clap(flatten)]
    parachain: runtime::cli::ConnectionOpts,

    /// Connection settings for Bitcoin Core.
    #[clap(flatten)]
    pub bitcoin: bitcoin::cli::BitcoinOpts, 

     /// Settings specific to the cli tool.
    #[clap(flatten)]
    config: ToolConfig,
}

#[derive(Parser, Clone)]
pub struct ToolConfig {
    /// Amount to redeem, in satoshis, 
    /// must be greater than Bridge Fee + BTC Network Fee + BTC Dust Limit 
    #[clap(long, validator = amount_gt_minimal, default_value = "15000")]
    max_redeem_amount: u128,

    /// Minimum wallet balance amount of wrapped token in sat, 
    /// bot will not trigger redeem when balance is below this amount
    #[clap(long, default_value = "5000")]
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
    let collateral_id  = parse_collateral_currency(&config.chain_collateral_id).unwrap();
    let wrapped_id  = parse_wrapped_currency(&config.chain_wrapped_id).unwrap();

    // User keys
    let (key_pair, _) = cli.account_info.get_key_pair()?;
    let signer = InterBtcSigner::new(key_pair);
    let signer_account_id = signer.account_id().clone();

    let btc_conf : Option<u32> = if cli.btc_network_confirmations > 0 {Some(cli.btc_network_confirmations)} else { Some(1)};

    // Connect to the parachain with the user keys
    let (shutdown_tx, _) = tokio::sync::broadcast::channel(16);
    let parachain_config = cli.parachain;
    tracing::trace!("{}",TEXT_CONNECT_ATTEMPT);
    let parachain = parachain_config.try_connect(signer.clone(), shutdown_tx.clone()).await?;
    tracing::info!("{}",TEXT_CONNECTED);
    tracing::info!("{}",TEXT_SEPARATOR);
 
    
    let native_id = parachain.get_native_currency_id();
    let mut balance_wrapped = parachain.get_free_balance_for_id(signer_account_id.clone(),wrapped_id).await?;
    let mut balance_collateral = parachain.get_free_balance_for_id(signer_account_id.clone(),collateral_id).await?;
    let mut balance_native = parachain.get_free_balance_for_id(signer_account_id.clone(),native_id).await?;
    let redeem_dust_amount = parachain.get_redeem_dust_amount().await?;

    // Connect to bitcoin core, setup wallet and get a receive address
    tracing::trace!("{}",TEXT_BTC_CONNECT_ATTEMPT);
    let bitcoin_config = cli.bitcoin;
     // let wallet_name = Some("PremiumBotWallet".to_string());
    // let prefix = wallet_name.clone().unwrap_or_else(|| "PremiumBotWallet".to_string());
    let bitcoin_core = bitcoin_config.new_client(Some(TEXT_BTC_BOT_WALLET.to_string())).await?;
    bitcoin_core.sync().await?;
    bitcoin_core.create_or_load_wallet().await?;
    tracing::trace!("{}",TEXT_BTC_WALLET_CONNECTED);
    let mut balance_btc = Amount::as_sat(bitcoin_core.get_balance(btc_conf)?); // Only 1 conf to get fast info on balance
    tracing::info!("{}",TEXT_BTC_CONNECTED);
    tracing::info!("{}",TEXT_SEPARATOR);



    tracing::info!("Parachain signer:           {}",signer_account_id.to_ss58check());
    tracing::info!("{} BTC confirmations required",btc_conf.unwrap());
    if cli.treat_all_vaults_as_premium > 0 {
        tracing::info!("Treat all vaults as premium (for testing)");
    };
    tracing::info!("Max redeem amount:          {} {} sat",config.max_redeem_amount, config.chain_wrapped_id);
    tracing::info!("Min wrapped balance:        {} {} sat",config.min_wrapped_balance, config.chain_wrapped_id);
    tracing::info!("Initial wrapped balance:    {} {} sat", balance_wrapped, config.chain_wrapped_id);
    tracing::info!("Initial collateral balance: {} {} planck", balance_collateral, config.chain_collateral_id);
    tracing::info!("Initial native balance:     {} {} planck", balance_native, get_currency_str(native_id.inner().unwrap()));
    tracing::info!("Initial BTC balance:        {} BTC sat", balance_btc);

    //Main loop
    // Check available wrapped balance
    // Identify Premium Redeem Vault
    // Request Redeem
    // Report KSM Gain
    // Repeat

    let mut loop_iteration : i32= 0;
    loop {
        loop_iteration = loop_iteration + 1;
        tracing::info!("[{}]{}",loop_iteration,TEXT_SEPARATOR);
        balance_wrapped = parachain.get_free_balance_for_id(signer_account_id.clone(),wrapped_id).await?;
        // Is there enough wrapped balance to proceed?
        let current_max_redeem_amount = if balance_wrapped < config.max_redeem_amount {
            balance_wrapped
        } else {
            config.max_redeem_amount
        };
        tracing::info!("Max {} redeem amount for this iteration: {} ", config.chain_wrapped_id, current_max_redeem_amount);
        if current_max_redeem_amount < config.min_wrapped_balance {
            tracing::warn!("{} balance (or max redeem amount) lower than minimum balance of {}  Sat", config.chain_wrapped_id, config.min_wrapped_balance);
            tracing::info!("Waiting {} seconds before checking again", config.sleeptime_not_enough_balance);
            let _sleep_result = sleep_with_parachain_ping(parachain.clone(), config.sleeptime_not_enough_balance).await;
            continue;
        } else {
            tracing::info!("Sufficient {} balance to attempt premium redeems", config.chain_wrapped_id);
        };

        // Are there some vaults with premium redeem available?
        let result = get_premium_redeem_vaults_or_all_active(parachain.clone(), cli.treat_all_vaults_as_premium).await;
        match &result {
            Ok(premium_vaults) => {
                if premium_vaults.len() == 0 { // This should not occur. RPC returns error instead
                    tracing::warn!("No premium redeem vault found");
                    tracing::info!("Waiting {} seconds before checking again", config.sleeptime_no_premium_vault);
                    let _sleep_result = sleep_with_parachain_ping(parachain.clone(), config.sleeptime_no_premium_vault).await;
                    continue;
                }
        
            }
            Err(error) => {
                let error_str = format!("{:?}",error); 
                if error_str.contains("Unable to find a vault below") {
                    tracing::warn!("No premium redeem vault found");
                    tracing::info!("Waiting {} seconds before checking again", config.sleeptime_no_premium_vault);
                    let _sleep_result = sleep_with_parachain_ping(parachain.clone(), config.sleeptime_no_premium_vault).await;
                    continue;  
                } else {
                    match error {
                        RuntimeError::VaultNotFound => {
                            tracing::warn!("No redeem vault found");
                            tracing::info!("Waiting {} seconds before checking again", config.sleeptime_no_premium_vault);
                            let _sleep_result = sleep_with_parachain_ping(parachain.clone(), config.sleeptime_no_premium_vault);
                            continue;  
                        },
                        _ => {
                            tracing::error!("Error when checking for premium vaults");
                            tracing::error!("{:?}",error);
                            tracing::info!("Waiting {} seconds before checking again", config.sleeptime_no_premium_vault);
                            let _sleep_result = sleep_with_parachain_ping(parachain.clone(), config.sleeptime_no_premium_vault).await;
                            continue;
                        }
                    }
                }
            }
        }

        let premium_vaults = result.unwrap();
        // select 1st vault with sufficient premium redeemable amount compared to configured current_max_redeem_amount
        // if none match the max_redeem_amount get the greatest amt
        let mut max_premium_amt = 0; 
        let mut index : usize = 0;
        let mut vault_index : usize = 0;

        for (_vault_id, loop_premium_amt) in premium_vaults.iter() {
            if loop_premium_amt.amount > current_max_redeem_amount {
                // Found eligible vault. use it
                // tracing::info!("Found. Index/Loop Amt/Vault_Index/max_premium_amt {}/{}/{}/{}",
                    //   index,loop_premium_amt.amount,vault_index, max_premium_amt);
                vault_index = index;
                break;
            };
            if max_premium_amt <= loop_premium_amt.amount {
                max_premium_amt = loop_premium_amt.amount;
                vault_index = index;

            }; 
            // tracing::info!("Search . Index/Loop Amt/Vault_Index/max_premium_amt {}/{}/{}/{}/{}",
            // index,loop_premium_amt.amount,vault_index, max_premium_amt, _vault_id.account_id.pretty_print());
            index = index + 1;
        };

        let (target_vault_id, premium_amt) =  &premium_vaults[vault_index];

        // Send redeem request
        let redeem_amount = if premium_amt.amount > current_max_redeem_amount {
            current_max_redeem_amount
        } else {
            premium_amt.amount
        };

        tracing::info!("Found vault {} with capacity {}", target_vault_id.account_id.pretty_print(), premium_amt.amount);

        // If  redeem_amount is less than redeem dust amount, sleep and retry later
        if redeem_amount <= redeem_dust_amount {
            tracing::info!("Redeem request amount  {} {} Sat is below dust level",
            redeem_amount,
            config.chain_wrapped_id);
            tracing::info!("Waiting {} seconds before checking again", config.sleeptime_no_premium_vault);
            let _sleep_result = sleep_with_parachain_ping(parachain.clone(), config.sleeptime_no_premium_vault).await;
            continue;
        } else {
            tracing::info!("Redeem request amount  {} {} Sat",
            redeem_amount,
            config.chain_wrapped_id);
        };
        let btc_address =get_new_btc_address(bitcoin_core.clone())?;
        let btc_address_str = btc_address.to_string();
        let btc_address_intr : BtcAddress = BtcAddress::decode_str(&btc_address_str).unwrap();
        tracing::info!("BTC receive address:        {}",btc_address);
                tracing::info!("Sending redeem request to parachain to vault {}", 
            target_vault_id.account_id.to_ss58check());
        let _redeem_id = parachain.request_redeem(redeem_amount, btc_address_intr, &target_vault_id).await?;
        tracing::info!("Parachain confirms redeem request of {} {} sat to BTC address {}",
                redeem_amount,
                config.chain_wrapped_id,
                btc_address_str
            );

            // TODO Find the txid via the redeem request info
        // // Wait for at least btc_conf confirmation of the BTC transaction to move on if asked for
        // if cli.wait_for_btc_confirmation > 0 {
        //     tracing::info!("Waiting for {} confirmations on bitcoin {} network", btc_conf.unwrap(), BITCOIN_NETWORK);
        //     let mut conf_prev: u32= 0;
        //     loop {
        //         let transaction_info = bitcoin_core.rpc.get_transaction(&txid, None)?;
        //         let conf = transaction_info.info.confirmations as u32;
        //         // Do some parachain activity to maintain rpc connection
        //         let _balance_native = parachain.get_free_balance_for_id(signer_account_id.clone(),native_id).await?;
        //         if conf != conf_prev {
        //             tracing::info!("Received {}/{} confirmations", conf, btc_conf.unwrap());
        //         };
        //         if conf >= btc_conf.unwrap() {
        //             break;
        //         };
        //         conf_prev = conf;
        //     }
        // };


        // Evaluate the reward by checking balances and reporting deltas
        let balance_wrapped_new = parachain.get_free_balance_for_id(signer_account_id.clone(),wrapped_id).await?;
        let balance_collateral_new = parachain.get_free_balance_for_id(signer_account_id.clone(),collateral_id).await?;
        let balance_native_new = parachain.get_free_balance_for_id(signer_account_id.clone(),native_id).await?;
        let delta_wrapped : i128 = balance_wrapped_new as i128 - balance_wrapped as i128;
        let delta_collateral : i128 = balance_collateral_new as i128 - balance_collateral as i128;
        let delta_native : i128 = balance_native_new as i128 - balance_native as i128;
        let balance_btc_new = Amount::as_sat(bitcoin_core.get_balance(btc_conf)?);
        let delta_btc : i128 = balance_btc_new as i128 - balance_btc as i128;
        tracing::info!("Wrapped balance:          {} {} sat", balance_wrapped_new, config.chain_wrapped_id);
        tracing::info!("Collateral balance:       {} {} planck", balance_collateral_new, config.chain_collateral_id);
        tracing::info!("Native balance:           {} {} planck", balance_native_new, get_currency_str(native_id.inner().unwrap()));
        tracing::info!("BTC balance:              {} BTC sat", balance_btc_new);
        tracing::info!("Delta wrapped balance:    {} {} sat", delta_wrapped, config.chain_wrapped_id);
        tracing::info!("Delta collateral balance: {} {} planck", delta_collateral, config.chain_collateral_id);
        tracing::info!("Delta native balance:     {} {} planck", delta_native, get_currency_str(native_id.inner().unwrap()));
        tracing::info!("Delta BTC balance:        {} BTC sat", delta_btc);
        // balance_wrapped = balance_wrapped_new;  // balance_wrapped checked at start of loop  
        balance_collateral = balance_collateral_new;
        balance_native = balance_native_new;
        balance_btc = balance_btc_new;

        tracing::info!("Waiting {} seconds before next loop iteration", config.sleeptime_main_loop);
        let _sleep_result = sleep_with_parachain_ping(parachain.clone(), config.sleeptime_main_loop).await;

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
                    if let VaultStatus::Active(active) = vault.status {
                        if active == true {
                            if let None = vault.banned_until { // Exclude banned vaults
                                let redeemable = vault.issued_tokens - vault.to_be_redeemed_tokens;
                                result.push((vault.id.clone(), BalanceWrapper { amount: redeemable }))
                            }
                        }
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


// let vault = parachain.get_vault(vault_id).await.unwrap()?;
// let banned = vault.banned_until; 
// if banned = None { //filters out banned vaults as of 1.11.2 
