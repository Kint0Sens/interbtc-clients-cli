use clap::Parser;
use git_version::git_version;
use std::convert::TryInto;
use bitcoin::{
    PartialAddress,
    BitcoinCoreApi,
    LockedTransaction,
    RpcApi,  // https://docs.rs/bitcoincore-rpc/0.14.0/bitcoincore_rpc/trait.RpcApi.html
    Amount,
};
use runtime::{
        IssuePallet,
        InterBtcSigner,
        VaultRegistryPallet,
        Ss58Codec,
        VaultId,
        AccountId,
        parse_collateral_currency,
        parse_wrapped_currency,
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

    /// Connection settings for Bitcoin Core.
    #[clap(flatten)]
    pub bitcoin: bitcoin::cli::BitcoinOpts, 
 
    /// Settings specific to the cli tool.
    #[clap(flatten)]
    config: ToolConfig,

    /// Confirmations needed for bitcoin balance checks and transfer check
    /// If omitted, defaults to 1. If set to 0 transfer completion will not be checked
    /// but balance checks will still use a default of 1
    #[clap(short, long, default_value = "1" )]
    btc_network_confirmations: u32,
}

#[derive(Parser, Clone)]
pub struct ToolConfig {
    /// Amount to issue, in satoshis
    #[clap(long)]
    amount: u128,

    /// Vault to issue from - account
    #[clap(long)]
    vault: AccountId,

    /// Vault to issue to - collateral
    #[clap(long, default_value = "KSM")] 
    vault_collateral_id: String,

    /// Vault to issue to
    #[clap(long, default_value = "KBTC")] 
    vault_wrapped_id: String,
}

#[tokio::main]
#[allow(unreachable_code)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli: Cli = Cli::parse();
    let config = cli.config;
    env_logger::init_from_env(init_logger(cli.verbose));

    if cli.testmode > 0 {
        tracing::info!("Running ni test mode, not signing transactions");
    }

    // User keys
    let (key_pair, _) = cli.account_info.get_key_pair()?;
    // let (ext,int,_) = cli.account_info.get_btc_keys()?;
    let signer = InterBtcSigner::new(key_pair);
    let signer_account_id = signer.account_id().clone();
    let collateral_id  = parse_collateral_currency(&config.vault_collateral_id).unwrap();
    let wrapped_id  = parse_wrapped_currency(&config.vault_wrapped_id).unwrap();
    let vault_id = VaultId::new(config.vault, collateral_id, wrapped_id);
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
    let bitcoin_core = bitcoin_config.new_client(Some(format!("InterbtcCLIWallet-master"))).await?;
    bitcoin_core.sync().await?;
    bitcoin_core.create_or_load_wallet().await?;
    let balance_btc = Amount::as_sat(bitcoin_core.get_balance(btc_conf)?);
    tracing::info!("{}",TEXT_BTC_CONNECTED);
    tracing::info!("{}",TEXT_SEPARATOR);

    let issuable_amount: u128 =  parachain.get_issuable_tokens_from_vault(vault_id.clone()).await?;

    tracing::info!("Signer:           {}",signer_account_id.to_ss58check());
    tracing::info!("Vault:            {}",vault_id.account_id.to_ss58check());
    tracing::info!("Issue amount:     {} {} sat",config.amount, config.vault_wrapped_id);
    tracing::info!("Issuable amount:  {} {} sat",issuable_amount, config.vault_wrapped_id);
    tracing::info!("BTC balance:      {} BTC sat",balance_btc);
   
    if config.amount > issuable_amount {
        tracing::error!("Insufficient issuable {} on Vault - Cancelling", config.vault_wrapped_id);
        return Ok(());
    };

    if config.amount > balance_btc.into() {
        tracing::error!("Insufficient BTC balance in BTC Wallet - Cancelling");
        return Ok(());
    };

    // Emit Issue Request
    tracing::info!("Sending issue request to parachain");
    let issue = parachain.request_issue(config.amount, &vault_id).await?;
    tracing::info!("Issue request accepted");
    // tracing::info!("Issue BTC address: {:?}",issue.vault_address);
    tracing::info!("Issue BTC address: {}",issue.vault_address.encode_str(BITCOIN_NETWORK).unwrap());
    
    tracing::info!("Issue amount:     {} {} sat",issue.amount, config.vault_wrapped_id);
    tracing::info!("Issue fee:        {} {} sat",issue.fee, config.vault_wrapped_id);

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

    Ok(())
     
    }

  