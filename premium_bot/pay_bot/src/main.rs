
use std::str::FromStr;


use clap::Parser;
use git_version::git_version;

use bitcoin::PartialAddress;
//interBTC related
use runtime::{
        IssuePallet,
        InterBtcSigner,
        Ss58Codec,
        // BtcAddress,
        AccountId,
        // CurrencyId,
        // parse_collateral_currency,
        // parse_wrapped_currency,
        };
use bdk::{
    bitcoin::Address, bitcoin::Network, blockchain::noop_progress, blockchain::ElectrumBlockchain,
    database::MemoryDatabase, electrum_client::Client, 
    // wallet::AddressIndex,
     Wallet, SignOptions,
};
use common::*;

const VERSION: &str = git_version!(args = ["--tags"]);
const AUTHORS: &str = env!("CARGO_PKG_AUTHORS");
const NAME: &str = env!("CARGO_PKG_NAME");
const ABOUT: &str = env!("CARGO_PKG_DESCRIPTION");

#[derive(Parser)]
#[clap(name = NAME, version = VERSION, author = AUTHORS, about = ABOUT)]
struct Cli {
    /// Simulation mode. Transaction not sent.
    #[clap(short, long, parse(from_occurrences))]
    testmode: usize,

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
    /// Vault to be used for issue
    /// If left empty, a random eligible vault will be selected
    #[clap(long)]
    force_vault_account_id: AccountId,

    /// Max Amount to issue, in satoshis, 
    /// must be greater than Bridge Fee + BTC Network Fee + BTC Dust Limit 
    #[clap(long, validator = amount_gt_minimal, default_value = "999999999999999999999")]
    max_issue_amount: u128,

    /// Min Amount to issue, in satoshis, 
    /// must be greater than Bridge Fee + BTC Network Fee + BTC Dust Limit 
    #[clap(long, validator = amount_gt_minimal, default_value = "2000")]
    min_issue_amount: u128,

    /// Minimum btc wallet amount in sat, 
    /// bot will not trigger issue when balance is below this amount
    #[clap(long, default_value = "2000")]
    min_btc_balance: u128,

    /// Sleep time before checking for available vault again
    #[clap(long, default_value = "15")]
    sleeptime_no_issuable_vault: u64,

    /// Sleep time before checking for available BTC again
    #[clap(long, default_value = "15")]
    sleeptime_not_enough_btc: u64,

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
    // Identify Vault with issuable capacity
    // Request Issue
    // Pay Issue
    // Report balance
    // repeat

    let amount = config.amount;
    // let btc_address : BtcAddress = BtcAddress::decode_str(&config.btc_address).unwrap();
    let collateral_id  = parse_collateral_currency(&config.vault_collateral_id).unwrap();
    let wrapped_id  = parse_wrapped_currency(&config.vault_wrapped_id).unwrap();
    let force_vault = if config.vault_account_id.is_empty() == true {
        true 
    } else {
        false
    };
    if force_vault == true { 
        let vault_id = VaultId::new(config.vault_account_id, collateral_id, wrapped_id);
    };

    // User keys
    let (key_pair, _) = cli.account_info.get_key_pair()?;
    let signer = InterBtcSigner::new(key_pair);
    let signer_account_id = signer.account_id().clone();


    // Connect to the parachain with the user keys
    let parachain_config = cli.parachain;
    let (shutdown_tx, _) = tokio::sync::broadcast::channel(16);
    tracing::trace!("TEXT_CONNECT_ATTEMPT");
    let parachain = parachain_config.try_connect(signer.clone(), shutdown_tx.clone()).await?;
    tracing::info!("TEXT_CONNECTED");


    tracing::info!("Signer:           {}",signer_account_id.to_ss58check());

    tracing::info!("Selected vault:            {}",vault_id.account_id.to_ss58check());
    tracing::info!("Issue amount:     {} {} Sat",config.amount, config.chain_wrapped_id);
    tracing::info!("Issuable amount:  {} {} Sat",issuable_amount, config.chain_wrapped_id);

  
    // Setup wallet
    let external_descriptor = "wpkh(tprv8ZgxMBicQKsPctgasNzABhRCAfReohQPdu235WxXhu7yuh3by91GhqZgRGN6GEdARTEWJ2iURcjtbAub8ifnzbym5vGs4V54DwK8VL9b9oZ/84'/0'/0'/0/*)";
    let internal_descriptor = "wpkh(tprv8ZgxMBicQKsPctgasNzABhRCAfReohQPdu235WxXhu7yuh3by91GhqZgRGN6GEdARTEWJ2iURcjtbAub8ifnzbym5vGs4V54DwK8VL9b9oZ/84'/0'/0'/1/*)";
    let wallet: Wallet<ElectrumBlockchain, MemoryDatabase> = Wallet::new(
        external_descriptor,
        Some(internal_descriptor),
        Network::Testnet,
        MemoryDatabase::new(),
        ElectrumBlockchain::from(Client::new("ssl://electrum.blockstream.info:60002").unwrap()),
    )?;

    // let address = wallet.get_address(AddressIndex::New)?;
    // tracing::info!("Generated Address: {}", address);

    // Main loop
    // loop {
    //     // Check available btc balance
    //     tracing::info!("Synching wallet");
    //     wallet.sync(noop_progress(), None)?;
    //     let balance = wallet.get_balance()?;
    //     tracing::info!("Wallet balance in SAT: {}", balance);
    //     if balance < min_btc_balance {
    //         // Not enough BTC. Sleep and retry later.
    //         tracing::warn!("Not enough BTC balance.");
    //         tracing::info!("Waiting {} seconds before checking again", config.sleeptime_not_enough_btc);
    //         thread::sleep(Duration::from_secs(config.sleeptime_not_enough_btc));
    //         continue;        
    //     };

    //     // Identify Vault with issuable capacity (or use forced vault)
    //     let vault_id;
    //     if force_vault == true {
    //         vault_id = force_vault_id;
    //     } else {
    //         // Get eligible vaults
    //         // Select one with issueable amount > max_issue_amount is found
    //         // else take the one with greatest issuable amt
    //         let vaults : Vec<_> = parachain.get_all_vaults().await?;
    //         let current_height = parachain.get_current_active_block_number().await?;
    //         let mut max_issuable_amt = 0; 
    //         let mut max_issuable_vault : VaultId;
    //         for vault in vaults.iter() {
    //             match vault.status {
    //                 VaultStatus::Active(active) => {
    //                     if active == false { // Vault set to inactive, does not accept issue requests
    //                         continue; 
    //                     };
    //                     let loop_issuable_amount: u128 =  parachain.get_issuable_tokens_from_vault(vault.clone()).await?;
    //                     if max_issuable_amt <= loop_issuable_amt.amount {
    //                         max_issuable_amt = loop_issuable_amt.amount;
    //                         max_issuable_vault = vault.clone();
    //                     }; 
                                    
    //                 }
    //                 _ => {},
    //         }

    //         if max_issuable_amt > config.min_issue_amount {
    //             vault_id = max_issuable_vault;
    //             tracing::info!("Selected vault {} with issuable amount of {} {}",
    //                             vault_id.id.account_id.pretty_print(),
    //                             max_issuable_amt,
    //                             wrapped_id );
    //         } else {
    //             // No vault found to execute issue. Sleep and retry later
    //             tracing::warn!("No vault available with issuable amount above minimum issue amount");
    //             tracing::info!("Waiting {} seconds before checking again", config.sleeptime_no_issuable_vault);
    //             thread::sleep(Duration::from_secs(config.sleeptime_no_issuable_vault));
    //             continue;
    //         };

    //     }
    
    //     // Emit Issue Request
    //     let issue_amount = if max_issue_amount > max_issuable_amt { max_issuable_amt } else { max_issue_amount };
    //     let issue = parachain.request_issue(issue_amount, &vault_id).await?;
    //     tracing::info!("Issue request accepted");
    //     tracing::info!("BTC address:      {:?}",issue.vault_address);
    //     tracing::info!("                  {:?}",issue.vault_address.encode_str(BITCOIN_NETWORK));
        
    //     tracing::info!("Issue amount:     {} {} Sat",issue.amount, config.vault_wrapped_id);
    //     tracing::info!("Issue fee:        {} {} Sat",issue.fee, config.vault_wrapped_id);

    //     // Send BTC transaction
    //     let tx_amount = issue.amount + issue.fee;
    //     let mut tx_builder = wallet.build_tx();
    //     tx_builder
    //         .add_recipient(issue_request_btc_address.script_pubkey(), tx_amount)
    //         .enable_rbf();
    //     let (mut psbt, tx_details) = tx_builder.finish()?;
    //     tracing::info!("Transaction details: {:#?}", tx_details);
    //     tracing::info!("Signing transaction");
    //     let finalized = wallet.sign(&mut psbt, SignOptions::default())?;
    //     assert!(finalized, "Tx has not been finalized");
    //     // tracing::info!("Transaction Signed: {}", finalized);
    //     let raw_transaction = psbt.extract_tx();
    //     let txid = wallet.broadcast(&raw_transaction)?;
    //     tracing::info!(
    //         "Transaction sent! TXID: {txid}.\nExplorer URL: https://blockstream.info/testnet/tx/{txid}",
    //         txid = txid
    //     );

    // }

    Ok(())
     
}

  