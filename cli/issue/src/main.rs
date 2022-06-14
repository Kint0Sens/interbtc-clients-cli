
use std::str::FromStr;


use clap::Parser;
use git_version::git_version;

//Tool code
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
    /// Vault to issue from - account
    #[clap(long)]
    vault_account_id: AccountId,

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
    env_logger::init_from_env(init_logger(cli.verbose));

    if cli.testmode > 0 {
        tracing::info!("Running ni test mode, not signing transactions");
    }

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

    tracing::info!("Synching local BTC wallet");
    wallet.sync(noop_progress(), None)?;
    let balance = wallet.get_balance()?;
    tracing::info!("BTC Wallet balance in SAT: {}", balance);
    let mut paid_issues = Vec::new();
    let mut found_one: bool;
    loop {
        found_one = false;
        let issue_requests = parachain.get_all_active_issues().await?;
        tracing::debug!("Found {} issues on target vault", issue_requests.len());
        for (issue_id, request) in issue_requests.into_iter() {
            if request.requester == signer_account_id {
                found_one = true;   
                tracing::info!("found issue id:{} for specified signer",issue_id);
                if paid_issues.contains(&issue_id) {
                    tracing::info!("issue id:{} - already paid, skipping",issue_id);
                } else {
                    tracing::info!("request_status:{:?}",request.status);
                    tracing::info!("request_btc:{:?}",request.btc_address);
                    // tracing::info!("request_btc_public_key:{:?}",request.btc_public_key);
                    tracing::info!("request_requester:{}",request.requester);
                    let amount : u64 = request.amount as u64;  // no checks, I do not have that many BTC
                    let fee : u64 = request.fee as u64;  // no checks, as above
                    tracing::info!("request_amount:{}, request_fee:{}",amount,fee);
                    let issue_request = parachain.get_issue_request(issue_id).await?;
                    let issue_request_btc_address_str = issue_request.btc_address.encode_str(BITCOIN_NETWORK).unwrap();
                    let issue_request_btc_address = Address::from_str(&issue_request_btc_address_str)?; 
                    tracing::info!("btc_address:{:?}",issue_request_btc_address_str);
        
                    tracing::info!("Synching wallet");
                    wallet.sync(noop_progress(), None)?;
                    let balance = wallet.get_balance()?;
                    tracing::info!("Wallet balance in SAT: {}", balance);

                    let tx_amount = amount + fee;

                    if balance < tx_amount {
                        tracing::info!("Balance too low. Cancelling payment");
                    } else {

                        let mut tx_builder = wallet.build_tx();
                        tx_builder
                            .add_recipient(issue_request_btc_address.script_pubkey(), tx_amount)
                            .enable_rbf();
                        let (mut psbt, tx_details) = tx_builder.finish()?;
                        tracing::info!("Transaction details: {:#?}", tx_details);
                        // Do not sign in test mode
                        if cli.testmode > 0 {
                            tracing::info!("Test mode. Not signing transaction");
                        } else {
                            tracing::info!("Signing transaction");
                            let finalized = wallet.sign(&mut psbt, SignOptions::default())?;
                            assert!(finalized, "Tx has not been finalized");
                            // tracing::info!("Transaction Signed: {}", finalized);
                            let raw_transaction = psbt.extract_tx();
                            let txid = wallet.broadcast(&raw_transaction)?;
                            tracing::info!(
                                "Transaction sent! TXID: {txid}.\nExplorer URL: https://blockstream.info/testnet/tx/{txid}",
                                txid = txid
                            );
                        }
                        paid_issues.push(issue_id);
                    }
                }
            } else {
                tracing::debug!("issue id:{} - other signer: {}",issue_id,request.requester);

            }
        }
        if found_one == false {
            tracing::info!("...Listenting to vault...");
        }
    };

    Ok(())
     
    }

  