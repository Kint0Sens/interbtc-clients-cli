use std::str::FromStr;
use std::convert::TryInto;
use clap::Parser;
use git_version::git_version;
use bitcoin::{
    PartialAddress,
    BitcoinCoreApi,
    LockedTransaction,
    RpcApi,  // https://docs.rs/bitcoincore-rpc/0.14.0/bitcoincore_rpc/trait.RpcApi.html
    Amount,
};
use runtime::{
    CollateralBalancesPallet,
    VaultRegistryPallet,
    IssuePallet,
    InterBtcSigner,
    Ss58Codec,
    PrettyPrint,        
    UtilFuncs,
    VaultId,
    VaultStatus,
    AccountId,
    CurrencyIdExt,
    parse_wrapped_currency,
    parse_collateral_currency,
};

use common::*;

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

    /// Wait for the parachain to confirm that the kbtc has been issued 
    #[clap(short, long, parse(from_occurrences))]
    wait_for_issued_kbtc: usize,

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
    /// Vault to issue to - account
    /// If not specified, an eligible vault is selected
    #[clap(long, default_value = "")] 
    vault_account_id: String,

    /// Max Amount to issue, in satoshis, 
    /// must be greater than Bridge Fee + BTC Network Fee + BTC Dust Limit 
    #[clap(long, validator = amount_gt_minimal, default_value = "15000")]
    max_issue_amount: u128,

    /// Minimum btc wallet amount in sat, 
    /// bot will not trigger issue when balance is below this amount.
    /// Must be greater than Bridge Fee + BTC Network Fee + BTC Dust Limit 
    /// also substracted from wallet BTC balance when tranferring maximum balance
    /// so as to avoid "Insufficient funds" error 
    #[clap(long, default_value = "5000")]
    min_btc_balance: u128,

    /// Sleep time before checking for available vault again
    #[clap(long, default_value = "15")]
    sleeptime_no_issuable_vault: u64,

    /// Sleep time before checking for available BTC again
    #[clap(long, default_value = "15")]
    sleeptime_not_enough_btc: u64,

    /// Sleep time after each succesful redeem loop
    #[clap(long, default_value = "15")]
    sleeptime_main_loop: u64,

    /// Sleep time wait for BTC trasfer completion
    #[clap(long, default_value = "120")]
    sleeptime_wait_for_btc_transfer: u64,


    /// Collateral
    #[clap(long, default_value = "KSM")]  // Make network dependent default
    chain_collateral_id: String,
 
    /// Wrapped
    #[clap(long, default_value = "KBTC")] // Make network dependent default
    chain_wrapped_id: String,
 }

#[tokio::main]
#[allow(unreachable_code)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli: Cli = Cli::parse();
    env_logger::init_from_env(init_logger(cli.verbose));

    let config = cli.config;

    //Main loop
    // Check available btc balance
    // Identify Vault with issuable capacity (or use vault entered in args)
    // Request Issue
    // Pay Issue
    // Report balances
    // Repeat

    // User keys
    let (key_pair, _) = cli.account_info.get_key_pair()?;
    let signer = InterBtcSigner::new(key_pair);
    let signer_account_id = signer.account_id().clone();
    let collateral_id  = parse_collateral_currency(&config.chain_collateral_id).unwrap();
    let wrapped_id  = parse_wrapped_currency(&config.chain_wrapped_id).unwrap();

    let btc_conf : Option<u32> = if cli.btc_network_confirmations > 0 {Some(cli.btc_network_confirmations)} else { Some(1)};


   // Connect to the parachain with the user keys
    let parachain_config = cli.parachain;
    let (shutdown_tx, _) = tokio::sync::broadcast::channel(16);
    tracing::trace!("{}",TEXT_CONNECT_ATTEMPT);
    let parachain = parachain_config.try_connect(signer.clone(), shutdown_tx.clone()).await?;
    tracing::info!("{}",TEXT_CONNECTED);
    tracing::info!("{}",TEXT_SEPARATOR);

    // Setup wallet
    tracing::trace!("{}",TEXT_BTC_CONNECT_ATTEMPT);
    let bitcoin_config = cli.bitcoin;
    // let wallet_name = Some("PremiumBotWallet".to_string());
    // let prefix = wallet_name.clone().unwrap_or_else(|| "PremiumBotWallet".to_string());
    let bitcoin_core = bitcoin_config.new_client(Some(TEXT_BTC_BOT_WALLET.to_string())).await?;
    bitcoin_core.sync().await?;
    bitcoin_core.create_or_load_wallet().await?;
    let mut balance_btc = Amount::as_sat(bitcoin_core.get_balance(btc_conf)?);
    tracing::info!("{}",TEXT_BTC_CONNECTED);
    tracing::info!("{}",TEXT_SEPARATOR);

    let native_id = parachain.get_native_currency_id();
    let use_forced_vault = if config.vault_account_id == "" { 
        tracing::info!("Automatic selection of vault");
        false 
    } else {
        tracing::info!("User specified vault: {}",config.vault_account_id);
        true 
    };
    
    let some_forced_vault_id : Option<VaultId> =  match use_forced_vault {
        true => {
            Some(VaultId::new(AccountId::from_str(&config.vault_account_id).unwrap(), collateral_id, wrapped_id ))
        },
        false => {
            None
        },
    };
  
    tracing::info!("Signer:                  {}",signer_account_id.to_ss58check());
    tracing::info!("Max issue amount:        {} {} sat",config.max_issue_amount, config.chain_wrapped_id);
    tracing::info!("Min BTC balance:         {} {} sat",config.min_btc_balance, config.chain_wrapped_id);
    tracing::info!("{} BTC confirmations required",btc_conf.unwrap());

    let mut balance_wrapped = parachain.get_free_balance_for_id(signer_account_id.clone(),wrapped_id).await?;
    let mut balance_native = parachain.get_free_balance_for_id(signer_account_id.clone(),native_id).await?;
    tracing::info!("Initial wrapped balance: {} {} sat", balance_wrapped, config.chain_wrapped_id);
    tracing::info!("Initial native balance:  {} {} planck", balance_native, get_currency_str(native_id.inner().unwrap()));
    tracing::info!("Initial BTC balance:     {} BTC sat", balance_btc);

    let mut loop_iteration : i32= 0;

    // Main loop
    loop {
        loop_iteration = loop_iteration + 1;
        tracing::info!("[{}]{}",loop_iteration,TEXT_SEPARATOR);
        let current_max_issue_amount = if u128::from(balance_btc) < ( config.max_issue_amount - config.min_btc_balance ) {
            (balance_btc as u128).clone().saturating_sub(config.min_btc_balance)
        } else {
            config.max_issue_amount.saturating_sub(config.min_btc_balance)
        };

        if current_max_issue_amount < config.min_btc_balance {
            // Not enough BTC. Sleep and retry later.
            tracing::warn!("Insufficient BTC balance:   {} BTC sat",balance_btc);
            tracing::info!("Waiting {} seconds before checking again", config.sleeptime_not_enough_btc);
            let _sleep_result = sleep_with_parachain_ping(parachain.clone(), config.sleeptime_not_enough_btc).await;
            balance_btc = Amount::as_sat(bitcoin_core.get_balance(btc_conf)?);
            let _sleep_result = sleep_with_parachain_ping(parachain.clone(), SHORT_SLEEP).await;
            continue; 
        } else {
            tracing::info!("Sufficient BTC balance to attempt issues");
            tracing::info!("Max BTC issue amount for this iteration: {} ", current_max_issue_amount);
        };

        // Identify Vault with issuable capacity (or use forced vault)
        // Get eligible vaults
        // Select one with issueable amount > max_issue_amount is found
        // else take the one with greatest issuable amt
        let mut max_issuable_amt = 0; 
        let issue_vault : VaultId;
        if use_forced_vault == true {
            issue_vault = some_forced_vault_id.clone().unwrap();
            max_issuable_amt = parachain.get_issuable_tokens_from_vault(issue_vault.clone()).await?;
        } else {
            let vaults : Vec<_> = parachain.get_all_vaults().await?;
            let mut index : usize = 0;
            let mut vault_index : usize = 0;
            for vault in vaults.iter() {
                match vault.status {
                    VaultStatus::Active(active) => {
// /                        tracing::trace!("{} Vault {} active: {}",index,vault.clone().id.account_id.pretty_print(), active);
                       
                        if active == false { // Vault set to inactive, does not accept issue requests
                            index = index + 1;
                            continue; 
                        };
                        let loop_issuable_amt: u128 =  parachain.get_issuable_tokens_from_vault(vault.id.clone()).await?;
                        if max_issuable_amt <= loop_issuable_amt {
                            max_issuable_amt = loop_issuable_amt;
                            vault_index = index;
                            // tracing::trace!("{} Max  {} Loop {}",index,max_issuable_amt,loop_issuable_amt);
                        }; 
                        index = index + 1;
                                        
                    },
                    _ => {},
                };
            };
            issue_vault = vaults[vault_index].id.clone();
            if max_issuable_amt > config.min_btc_balance {
                tracing::info!("Selected vault {} with issuable amount of {} {}",
                                issue_vault.account_id.pretty_print(),
                                max_issuable_amt,
                                config.chain_wrapped_id );
            };
        };    
        if max_issuable_amt < config.min_btc_balance {
            // No vault found to execute issue. Sleep and retry later
            tracing::warn!("No vault available with issuable amount above minimum issue amount");
            tracing::info!("Waiting {} seconds before checking again", config.sleeptime_no_issuable_vault);
            let _sleep_result = sleep_with_parachain_ping(parachain.clone(), config.sleeptime_no_issuable_vault).await;
            continue;
        };

        // Emit Issue Request
        let issue_amount = if current_max_issue_amount > max_issuable_amt { max_issuable_amt } else { current_max_issue_amount };
        let issue = parachain.request_issue(issue_amount, &issue_vault).await?;
        tracing::info!("Sending issue request for {} BTC sat to parachain", issue_amount);
        tracing::info!("Issue request accepted");
        // tracing::info!("Issue BTC address: {:?}",issue.vault_address);
        tracing::info!("Issue BTC address: {}",issue.vault_address.encode_str(BITCOIN_NETWORK).unwrap());
        
        tracing::info!("Issue amount:     {} {} sat",issue.amount, config.chain_wrapped_id);
        tracing::info!("Issue fee:        {} {} sat",issue.fee, config.chain_wrapped_id);

        tracing::info!("Building BTC transaction");
        // Send BTC transaction
        let tx_amount: u64 = (issue.amount as u128 + issue.fee as u128).try_into().unwrap();
        // let mut tx_builder = wallet.build_tx();
        let issue_request_btc_address_str = issue.vault_address.encode_str(BITCOIN_NETWORK).unwrap();
        // let issue_request_btc_address = Address::from_str(&issue_request_btc_address_str)?; 

        // Create raw trasaction
        let raw_tx = bitcoin_core.create_raw_transaction_hex(issue_request_btc_address_str.clone(), Amount::from_sat(tx_amount), None)?; //Amount::as_sat
        // fund the transaction: adds required inputs, and possibly a return-to-self output
        let funded_raw_tx = bitcoin_core.rpc.fund_raw_transaction(raw_tx, None, None)?;
        // sign the transaction
        let signed_funded_raw_tx =
        bitcoin_core.rpc
            .sign_raw_transaction_with_wallet(&funded_raw_tx.transaction()?, None, None)?;
        // Make sure signing is successful
        if signed_funded_raw_tx.errors.is_some() {
            tracing::info!("Transaction Signing Error");
        }
        let transaction = signed_funded_raw_tx.transaction()?;
        let tx = LockedTransaction::new(transaction, issue_request_btc_address_str.clone(), None);
        let txid = bitcoin_core.send_transaction(tx).await?;
        tracing::info!("Transaction sent. TxID: {}",txid);

        // Evaluate the balances and report deltas

        // Optional loop to wait for BTC transfer completion
        let mut balance_wrapped_new: u128;
        let mut delta_wrapped : i128;
        loop {
            balance_wrapped_new = parachain.get_free_balance_for_id(signer_account_id.clone(),wrapped_id).await?;
            delta_wrapped  = balance_wrapped_new as i128 - balance_wrapped as i128;
            if cli.wait_for_issued_kbtc == 0 {
                tracing::info!("Not waiting for parachain KBTC issue confirmation. {} balance and delta migth be incorrect",config.chain_wrapped_id);
                break;
            }
            if delta_wrapped != 0 {
                break;
            }
            tracing::info!("Waiting {} seconds for parachain KBTC issue confirmation", config.sleeptime_wait_for_btc_transfer);
            let _sleep_result = sleep_with_parachain_ping(parachain.clone(), config.sleeptime_wait_for_btc_transfer).await;
        }


        // Wait for at least btc_conf confirmation of the BTC transaction to move on
        if cli.btc_network_confirmations > 0 {
            tracing::info!("Waiting for {} confirmations on bitcoin {} network", cli.btc_network_confirmations, BITCOIN_NETWORK);
            let mut conf_prev: u32= 0;
            loop {
                let transaction_info = bitcoin_core.rpc.get_transaction(&txid, None)?;
                let conf = transaction_info.info.confirmations as u32;
                let _sleep_result = sleep_with_parachain_ping(parachain.clone(), SHORT_SLEEP).await;
                if conf != conf_prev {
                    tracing::info!("Received {}/{} confirmations", conf, btc_conf.unwrap());
                };
                if conf >= cli.btc_network_confirmations {
                    break;
                };
                conf_prev = conf;
            }
        } else {
            tracing::info!("Not waiting for {} network transaction confirmation",BITCOIN_NETWORK);
        };

        let balance_native_new = parachain.get_free_balance_for_id(signer_account_id.clone(),native_id).await?;
        let delta_native : i128 = balance_native_new as i128 - balance_native as i128;
        let balance_btc_new = Amount::as_sat(bitcoin_core.get_balance(btc_conf)?);
        let delta_btc : i128 = balance_btc_new as i128 - balance_btc as i128;
        tracing::info!("Wrapped balance:       {} {} sat", balance_wrapped_new, config.chain_wrapped_id);
        tracing::info!("Native balance:        {} {} planck", balance_native_new, get_currency_str(native_id.inner().unwrap()));
        tracing::info!("BTC balance:           {} BTC sat", balance_btc_new);
        tracing::info!("Delta wrapped balance: {} {} sat", delta_wrapped, config.chain_wrapped_id);
        tracing::info!("Delta native balance:  {} {} planck", delta_native, get_currency_str(native_id.inner().unwrap()));
        tracing::info!("Delta BTC balance:     {} BTC sat", delta_btc);
        balance_wrapped = balance_wrapped_new;
        balance_native = balance_native_new;
        balance_btc = balance_btc_new;

        tracing::info!("Waiting {} seconds before next loop iteration", config.sleeptime_main_loop);
        let _sleep_result = sleep_with_parachain_ping(parachain.clone(), config.sleeptime_main_loop).await;

    };
    Ok(())  
}