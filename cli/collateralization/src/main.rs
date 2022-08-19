use std::env;
use clap::Parser;
// use clap::Subcommand;
use git_version::git_version;
// use tabular::{Table, Row};

//interBTC related
use runtime::InterBtcSigner;
use runtime::VaultRegistryPallet;
use runtime::CurrencyIdExt;
use runtime::CurrencyInfo;
use runtime::VaultStatus;
use runtime::SecurityPallet;
use runtime::PrettyPrint;
// use runtime::parse_collateral_currency;
// use runtime::parse_wrapped_currency;

use common::*;

const VERSION: &str = git_version!(args = ["--tags"]);
const AUTHORS: &str = env!("CARGO_PKG_AUTHORS");
const NAME: &str = env!("CARGO_PKG_NAME");
const ABOUT: &str = env!("CARGO_PKG_DESCRIPTION");

// #[derive(Subcommand)]
// enum Commands {
//     // Add { name: Option<String> },
//     Vaults,
//     // Redeems,
//     // Issues,
//     LiquidationVault,
// }

    // // Get list of premium redeem Vaults
    // let result = parachain.get_premium_redeem_vaults().await;
    // //  let result = parachain.get_vaults_with_issuable_tokens().await?;
     
    //  tracing::info!("Call done.");
    //  match  result {
    //      Ok(premium_redeem_vaults) => tracing::info!("{:#?}",premium_redeem_vaults[0]),
    //      Err(error) => tracing::info!("Error returned: {:?}",error),
    //  };

#[derive(Parser)]
#[clap(name = NAME, version = VERSION, author = AUTHORS, about = ABOUT)]
struct Cli {
    /// Return all logs 
    /// Overridden by RUST_LOG env variable
    #[clap(short = 'v', long, parse(from_occurrences))]
    verbose: usize,

    // /// Subcommand
    // #[clap(subcommand)]
    // command: Commands,

    // /// Vaults subcommand filter. Filter on Premium Redeem vaults
    // #[clap(short = 'P', long, parse(from_occurrences))]
    // premium: usize,

    // /// Vaults subcommand filter. Filter on Active vaults
    // #[clap(short = 'A', long, parse(from_occurrences))]
    // active: usize,

    /// Keyring / keyfile options.
    #[clap(flatten)]
    account_info: runtime::cli::ProviderUserOpts,

//    /// Liquidation Vault - collateral token
//    #[clap(long, default_value = "KSM")] 
//    vault_collateral_id: String,

//    /// Liquidation Vault - wrapped token
//    #[clap(long, default_value = "KBTC")] 
//    vault_wrapped_id: String,

    /// Connection settings for the BTC Parachain.
    #[clap(flatten)]
    parachain: runtime::cli::ConnectionOpts,
}

#[derive(Default)]
struct CollateralizationData {
    vault : String,
    pending_wrapped : u128, 
    pending_collateralization : u128,
    pending_collateral_in_wrapped : u128
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let cli: Cli = Cli::parse();
    env_logger::init_from_env(init_logger(cli.verbose));

    let (key_pair, _) = cli.account_info.get_key_pair()?;
    let signer = InterBtcSigner::new(key_pair);
    
    let parachain_config = cli.parachain;
    let (shutdown_tx, _) = tokio::sync::broadcast::channel(16);
    let parachain = parachain_config.try_connect(signer.clone(), shutdown_tx.clone()).await?;
    println!("{}",TEXT_CONNECTED);
    println!("{}",TEXT_SEPARATOR);



    
        let vaults : Vec<_> = parachain
        .get_all_vaults()
        .await?;

        let current_height = parachain.get_current_active_block_number().await?;

        println!("Found {} vaults. Current block {}",vaults.len(), current_height);

        let mut lowest_collateralization : u128 = u128::MAX;
        let mut collateralization_data : Vec<CollateralizationData> = Vec::new();
        for vault in vaults.iter() {

            let mut collateralization_record : CollateralizationData = CollateralizationData::default();
            collateralization_record.vault = vault.id.account_id.pretty_print().to_string();

            println!("");
            println!("{}",TEXT_SEPARATOR);
            println!("{}",vault.id.account_id.pretty_print());

            match vault.status {
                VaultStatus::Active(active) => { 
                    // Banned info for active vaults
                    let banned  : String = match vault.banned_until {
                        None => {
                            "".to_string()
                        },
                        Some(until) => {
                            match active {
                                true => { format!(" - Banned until {}",until) }, 
                                false => { "".to_string() }, 
                            }
                        },
                    };

                    // Check if accepts issues/redeems (active)
                    let inactive : String = match active {
                        true => { "Active".to_string() }, 
                        false => { "Inactive".to_string() }, 
                    };

                    println!("   {}{}", inactive, banned);

                    let total_collateral = parachain
                    .get_vault_total_collateral(vault.id.clone())
                    .await?;
                    let total_collateral_str = pretty_print_currency_amount(total_collateral, vault.id.currencies.collateral).unwrap();
                    println!("   Collateral                 {} {}", total_collateral_str, vault.id.currencies.collateral.inner().unwrap().symbol());


                    let issued_tokens_str = pretty_print_currency_amount(vault.issued_tokens, vault.id.currencies.wrapped).unwrap();
                    let to_be_issued_tokens_str = pretty_print_currency_amount(vault.to_be_issued_tokens, vault.id.currencies.wrapped).unwrap();
                    let to_be_redeemed_tokens_str = pretty_print_currency_amount(vault.to_be_redeemed_tokens, vault.id.currencies.wrapped).unwrap();
                    let to_be_replaced_tokens_str = pretty_print_currency_amount(vault.to_be_replaced_tokens, vault.id.currencies.wrapped).unwrap();
        
                    let all_tokens = vault.issued_tokens 
                                    + vault.to_be_issued_tokens 
                                    - vault.to_be_redeemed_tokens 
                                    - vault.to_be_replaced_tokens;
                    let all_tokens_str = pretty_print_currency_amount(all_tokens, vault.id.currencies.wrapped).unwrap();
                    println!("   Tokens - Current           {} {}", issued_tokens_str, vault.id.currencies.wrapped.inner().unwrap().symbol());
                    println!("   Tokens - Pending           {} {}", all_tokens_str, vault.id.currencies.wrapped.inner().unwrap().symbol());
                    println!("   Tokens - To be issued      {} {}", to_be_issued_tokens_str, vault.id.currencies.wrapped.inner().unwrap().symbol());
                    println!("   Tokens - To be redeemed    {} {}", to_be_redeemed_tokens_str, vault.id.currencies.wrapped.inner().unwrap().symbol());
                    println!("   Tokens - To be replaced    {} {}", to_be_replaced_tokens_str, vault.id.currencies.wrapped.inner().unwrap().symbol());
    
                    collateralization_record.pending_wrapped = all_tokens;
;
                    // let issuable_amount: u128 =  parachain.get_issuable_tokens_from_vault(vault.id.clone()).await?;
                    // let issuable_tokens_str = pretty_print_currency_amount(issuable_amount, vault.id.currencies.wrapped).unwrap();
                    // let collateralization_issued : String = match parachain
                    // .get_collateralization_from_vault(vault.id.clone(),true)
                    // .await {
                    //     Ok(collateralization) => {
                    //         pretty_print_planck_amount(collateralization, 16).unwrap()
                    //     },
                    //     Err(err) => {
                    //         // If issued tokens are = 0 assume it is a / by 0 err
                    //         if vault.issued_tokens == 0 {
                    //             "0".to_string()
                    //         } else {
                    //             tracing::error!("Error getting collateralization: {}", err);
                    //             "0".to_string()
                    //         }
                    //     },
                    // };

                    let collateralization_all : String = match parachain
                    .get_collateralization_from_vault(vault.id.clone(),false)
                    .await {
                        Ok(collateralization) => {
                            if  collateralization < lowest_collateralization  {
                                lowest_collateralization = collateralization;
                            }
                            pretty_print_planck_amount(collateralization, 16).unwrap()
                        },
                        Err(err) => {
                            // If issued tokens + to_be_issued_tokens are = 0 assume it is a / by 0 err
                            let all_issued_tokens = vault.issued_tokens + vault.to_be_issued_tokens;
                            if all_issued_tokens == 0 {
                                "0".to_string()
                            } else {
                                tracing::error!("Error getting collateralization: {}", err);
                            "0".to_string()
                            }
                        },
                    };

                    // println!("   Collateralization - issued {}", collateralization_issued);
                    println!("   Collateralization - all    {}", collateralization_all);
                    // println!("   Issuable tokens:           {} {}", issuable_tokens_str, vault.id.currencies.wrapped.inner().unwrap().symbol());

                    collateralization_record.pending_collateralization = collateralization_all;
                    collateralization_record.pending_collateral_in_wrapped = collateralization_all / 10000000000000000 ;
                    collateralization_record.pending_collateral_in_wrapped =
                        collateralization_record.pending_collateral_in_wrapped * collateralization_record.pending_wrapped;

                    
                    },
                VaultStatus::Liquidated => {
                    println!("Liquidated");
                },
                // VaultStatus::CommittedTheft => { 
                //     println!("CommittedTheft");
                // },

            }

            collateralization_data.push(collateralization_record);
        
        }

        let lowest_collateralization_all = pretty_print_planck_amount(lowest_collateralization, 16).unwrap();
        println!("   Lowest Collateralization (pending)    {}", lowest_collateralization_all);
                        
 
    
     Ok(())
     
    }

