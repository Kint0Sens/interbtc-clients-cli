use clap::Parser;
use git_version::git_version;


//interBTC related
use runtime::{
        IssuePallet,
        InterBtcSigner,
        VaultRegistryPallet,
        Ss58Codec,
        // BtcAddress,
        VaultId,
        AccountId,
        // CurrencyId,
        parse_collateral_currency,
        parse_wrapped_currency,
        };
use bitcoin::PartialAddress;
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
 
    /// Testmode
    #[clap(short, long, parse(from_occurrences))]
    testmode: usize,

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
   /// Amount to issue, in satoshis
    #[clap(long)]
    amount: u128,

    /// Vault to issue from - account
    #[clap(long)]
    vault_account_id: AccountId,

    /// Vault to redeem from - collateral
    #[clap(long, default_value = "KSM")] 
    vault_collateral_id: String,

    /// Vault to redeem from - wrapped currency
    #[clap(long, default_value = "KBTC")] 
    vault_wrapped_id: String,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let cli: Cli = Cli::parse();
    env_logger::init_from_env(init_logger(cli.verbose));

    let config = cli.config;

    if cli.testmode > 0 {
        tracing::info!("Test mode");
    }

    let amount = config.amount;
    // let btc_address : BtcAddress = BtcAddress::decode_str(&config.btc_address).unwrap();
    let collateral_id  = parse_collateral_currency(&config.vault_collateral_id).unwrap();
    let wrapped_id  = parse_wrapped_currency(&config.vault_wrapped_id).unwrap();
    let vault_id = VaultId::new(config.vault_account_id, collateral_id, wrapped_id);

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

    let issuable_amount: u128 =  parachain.get_issuable_tokens_from_vault(vault_id.clone()).await?;

    tracing::info!("Signer:           {}",signer_account_id.to_ss58check());
    tracing::info!("Vault:            {}",vault_id.account_id.to_ss58check());
    tracing::info!("Issue amount:     {} {} Sat",config.amount, config.vault_wrapped_id);
    tracing::info!("Issuable amount:  {} {} Sat",issuable_amount, config.vault_wrapped_id);
    if config.amount > issuable_amount {
        tracing::error!("Insufficient issuable {} on Vault - Cancelling", config.vault_wrapped_id);
        return Ok(());
    };

    if cli.testmode > 0 {
        tracing::info!("Test mode, skipping request_issue call");
    } else {

        tracing::info!("Issue request emitted");
        let issue = parachain.request_issue(amount, &vault_id).await?;
        tracing::info!("Issue request accepted");
        tracing::info!("BTC address:      {:?}",issue.vault_address);
        tracing::info!("                  {:?}",issue.vault_address.encode_str(BITCOIN_NETWORK));
        
        tracing::info!("Issue amount:     {} {} Sat",issue.amount, config.vault_wrapped_id);
        tracing::info!("Issue fee:        {} {} Sat",issue.fee, config.vault_wrapped_id);
    
        // Get all active issues on the vault
        let issue_requests = parachain.get_all_active_issues().await?;
        for (issue_id, request) in issue_requests.into_iter() {
            if request.requester == signer_account_id {   
                tracing::info!("issue id:{}",issue_id);
                tracing::info!("request_status:{:?}",request.status);
                tracing::info!("request_btc:{:?}",request.btc_address);
                tracing::info!("request_btc_public_key:{:?}",request.btc_public_key);
                tracing::info!("request_requester:{}",request.requester);
                tracing::info!("request_amount:{}",request.amount);



                let issue_request = parachain.get_issue_request(issue_id).await?;
                tracing::info!("btc_address:{:?}",issue_request.btc_address);
                tracing::info!("btc_address bis:{:?}",issue_request.btc_address.encode_str(BITCOIN_NETWORK));
            }
        }
    }

    Ok(())
     
    }

