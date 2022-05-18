mod error;

use std::env;
use clap::Parser;
use clap::Subcommand;
use git_version::git_version;
use tabular::{Table, Row};

//interBTC related
use error::Error;
use runtime::InterBtcSigner;
use runtime::VaultRegistryPallet;
use runtime::CurrencyIdExt;
use runtime::CurrencyInfo;
use runtime::VaultStatus;
use runtime::SecurityPallet;
use runtime::PrettyPrint;
use runtime::parse_collateral_currency;
use runtime::parse_wrapped_currency;

use common::*;

const VERSION: &str = git_version!(args = ["--tags"]);
const AUTHORS: &str = env!("CARGO_PKG_AUTHORS");
const NAME: &str = env!("CARGO_PKG_NAME");
const ABOUT: &str = env!("CARGO_PKG_DESCRIPTION");

#[derive(Subcommand)]
enum Commands {
    // Add { name: Option<String> },
    Vaults,
    // Redeems,
    // Issues,
    LiquidationVault,
}

#[derive(Parser)]
#[clap(name = NAME, version = VERSION, author = AUTHORS, about = ABOUT)]
struct Cli {
    /// Return all logs 
    /// Overridden by RUST_LOG env variable
    #[clap(short = 'v', long, parse(from_occurrences))]
    verbose: usize,

    /// Subcommand
    #[clap(subcommand)]
    command: Commands,

    /// Vaults subcommand filter. Filter on Premium Redeem vaults
    #[clap(short = 'P', long, parse(from_occurrences))]
    premium: usize,

    /// Vaults subcommand filter. Filter on Active vaults
    #[clap(short = 'A', long, parse(from_occurrences))]
    active: usize,

    /// Keyring / keyfile options.
    #[clap(flatten)]
    account_info: runtime::cli::ProviderUserOpts,

   /// Liquidation Vault - collateral token
   #[clap(long, default_value = "KSM")] 
   vault_collateral_id: String,

   /// Liquidation Vault - wrapped token
   #[clap(long, default_value = "KBTC")] 
   vault_wrapped_id: String,

    /// Connection settings for the BTC Parachain.
    #[clap(flatten)]
    parachain: runtime::cli::ConnectionOpts,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let cli: Cli = Cli::parse();
    env_logger::init_from_env(init_logger(cli.verbose));

    let (key_pair, _) = cli.account_info.get_key_pair()?;
    let signer = InterBtcSigner::new(key_pair);
    
    let parachain_config = cli.parachain;
    let (shutdown_tx, _) = tokio::sync::broadcast::channel(16);
    tracing::trace!("TEXT_CONNECT_ATTEMPT");
    let parachain = parachain_config.try_connect(signer.clone(), shutdown_tx.clone()).await?;
    tracing::info!("TEXT_CONNECTED");

    match &cli.command {
        Commands::Vaults => {
            // get all vaults will exclude TheftDetected and Liquidated vaults
            // TODO change to a custom
            let vaults : Vec<_> = parachain
            .get_all_vaults_really_all()
            .await?;
    
            let current_height = parachain.get_current_active_block_number().await?;

            println!("Found {} vaults. Current block {}",vaults.len(), current_height);
    
            for vault in vaults.iter() {

                println!("{}",vault.id.account_id.pretty_print());
                let total_collateral = parachain
                .get_vault_total_collateral(vault.id.clone())
                .await?;
                let total_collateral_str = pretty_print_currency_amount(total_collateral, vault.id.currencies.collateral).unwrap();
                println!("   Collateral {}", total_collateral_str);

                let issued_tokens_str = pretty_print_currency_amount(vault.issued_tokens, vault.id.currencies.wrapped).unwrap();
 
                let all_tokens = vault.issued_tokens 
                               + vault.to_be_issued_tokens 
                               - vault.to_be_redeemed_tokens
                               - vault.to_be_replaced_tokens;
                let all_tokens_str = pretty_print_currency_amount(all_tokens, vault.id.currencies.wrapped).unwrap();
                println!("   Tokens (Current / Pending) {} {}", issued_tokens_str, all_tokens_str);



                match vault.status {
                    VaultStatus::Active(active) => { 

                        let issuable_amount: u128 =  parachain.get_issuable_tokens_from_vault(vault.id.clone()).await?;
                        let collateralization_issued : String = match parachain
                        .get_collateralization_from_vault(vault.id.clone(),true)
                        .await {
                            Ok(collateralization) => {
                                pretty_print_planck_amount(collateralization, 16).unwrap()
                            },
                            Err(err) => {
                                // If issued tokens are = 0 assume it is a / by 0 err
                                if vault.issued_tokens == 0 {
                                    "0".to_string()
                                } else {
                                    tracing::error!("Error getting collateralization: {}", err);
                                    "0".to_string()
                                }
                            },
                        };
                        let collateralization_all : String = match parachain
                        .get_collateralization_from_vault(vault.id.clone(),false)
                        .await {
                            Ok(collateralization) => {
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
                        // Banned info for active vaults
                        let banned  : String = match vault.banned_until {
                                None => {
                                    "".to_string()
                                },
                                Some(until) => {
                                     format!(" - Banned until {}",until)
                                },
                            };

                        // Check if accepts issues/redeems (active)
                        let inactive : String = match active {
                                true => { "- Active".to_string() }, 
                                false => { "- Inactive".to_string() }, 
                            };

                        println!("   {}{}", inactive, banned);
                        println!("   Collateralization : {} / {}",  collateralization_issued, collateralization_all);
                        println!("   Issuable tokens: {}", issuable_amount);
                     },
                    VaultStatus::Liquidated => {
                        println!("Liquidated");
                    },
                    VaultStatus::CommittedTheft => { 
                        println!("CommittedTheft");
                    },
                }


            }
   
        }
        Commands::LiquidationVault => {
            let collateral_id  = parse_collateral_currency(&cli.vault_collateral_id).unwrap();
            let wrapped_id  = parse_wrapped_currency(&cli.vault_wrapped_id).unwrap();

            let liquidation_vault = parachain
            .get_liquidation_vault(collateral_id, wrapped_id)
            .await?;

            let collateral_str = pretty_print_currency_amount(liquidation_vault.collateral, liquidation_vault.currency_pair.collateral).unwrap();

            let burnable = liquidation_vault.issued_tokens - liquidation_vault.to_be_redeemed_tokens;
            let burnable_str = pretty_print_currency_amount(burnable, liquidation_vault.currency_pair.wrapped).unwrap();


            let user_receives_per_token_denom = liquidation_vault.issued_tokens + liquidation_vault.to_be_issued_tokens;
            let mut user_receives_per_token_f : f32 = liquidation_vault.collateral as f32 / user_receives_per_token_denom as f32; 

            // let mut user_receives_per_token = liquidation_vault.collateral;
            if  liquidation_vault.currency_pair.collateral.inner().decimals() > liquidation_vault.currency_pair.wrapped.inner().decimals()  {
                let decimals = liquidation_vault.currency_pair.collateral.inner().decimals() as u32
                - liquidation_vault.currency_pair.wrapped.inner().decimals() as u32;
                // let user_receives_per_token_decimals = 10_u128.pow(decimals); 
                let user_receives_per_token_decimals_f = 10_f32.powf(decimals as f32); 
                // user_receives_per_token = user_receives_per_token.checked_div(user_receives_per_token_decimals).unwrap();
                user_receives_per_token_f = user_receives_per_token_f / user_receives_per_token_decimals_f;

            } else if liquidation_vault.currency_pair.collateral.inner().decimals() < liquidation_vault.currency_pair.wrapped.inner().decimals()  {
                let decimals = liquidation_vault.currency_pair.wrapped.inner().decimals() as u32
                - liquidation_vault.currency_pair.collateral.inner().decimals() as u32;
                // let user_receives_per_token_decimals = 10_u128.pow(decimals); 
                let user_receives_per_token_decimals_f = 10_f32.powf(decimals as f32); 
                // user_receives_per_token = user_receives_per_token.checked_mul(user_receives_per_token_decimals).unwrap();
                user_receives_per_token_f = user_receives_per_token_f * user_receives_per_token_decimals_f;
            };
            // user_receives_per_token = user_receives_per_token.checked_div(user_receives_per_token_denom).unwrap_or(0);
    

            let issued_tokens_str = pretty_print_currency_amount(liquidation_vault.issued_tokens, liquidation_vault.currency_pair.wrapped).unwrap();
            let to_be_issued_tokens_str = pretty_print_currency_amount(liquidation_vault.to_be_issued_tokens, liquidation_vault.currency_pair.wrapped).unwrap();
            let to_be_redeemed_tokens_str = pretty_print_currency_amount(liquidation_vault.to_be_redeemed_tokens, liquidation_vault.currency_pair.wrapped).unwrap();

            let mut table = Table::new("   {:<}         {:>}  {:<} {:<}");
            table
                .add_heading(format!("Liquidation Vault [{}/{}]",
                    liquidation_vault.currency_pair.collateral.inner().symbol(),
                    liquidation_vault.currency_pair.wrapped.inner().symbol()
                    )
                )
                .add_heading("--------------------------------")
                .add_row(Row::new()
                    .with_cell("Collateral")
                    .with_cell(collateral_str)
                    .with_cell(liquidation_vault.currency_pair.collateral.inner().symbol())
                    .with_cell("")
                )
                .add_row(Row::new()
                    .with_cell("Burnable")
                    .with_cell(burnable_str)
                    .with_cell(liquidation_vault.currency_pair.wrapped.inner().symbol())
                    .with_cell("- (burnable = issued - to_be_redeemed)")
                )
                .add_row(Row::new()
                    .with_cell("Reward per burnt token")
                    .with_cell(format!("{}",user_receives_per_token_f))
                    .with_cell(liquidation_vault.currency_pair.collateral.inner().symbol())
                    .with_cell("- (collateral / (issued_tokens + to_be_issued_tokens))")
                )
                .add_heading("--------------------------------")
                .add_row(Row::new()
                    .with_cell("to_be_issued_tokens")
                    .with_cell(to_be_issued_tokens_str)
                    .with_cell(liquidation_vault.currency_pair.wrapped.inner().symbol())
                    .with_cell("")
                )
                .add_row(Row::new()
                    .with_cell("issued_tokens")
                    .with_cell(issued_tokens_str)
                    .with_cell(liquidation_vault.currency_pair.wrapped.inner().symbol())
                    .with_cell("")
                )
                .add_row(Row::new()
                    .with_cell("to_be_redeemed_tokens")
                    .with_cell(to_be_redeemed_tokens_str)
                    .with_cell(liquidation_vault.currency_pair.wrapped.inner().symbol())
                    .with_cell("")
                );
            print!("{}", table);

        }
        // Commands::Issues => {}
        // Commands::Redeems => {}
    };    
    
    // let addr2vaults : Vec<_> = vaults
    //         .into_iter()
    //         .flat_map(|vault| {
    //             tracing::info!("Vault:{} - {:#?}",vault.id.clone(),vault.clone());
    //             vault
    //                 .wallet
    //                 .addresses
    //                 .iter()
    //                 .map(|addr| {
    //                     (*addr, vault.id.clone());
    //                     // tracing::info!("Wallet address:{:#?}",*addr);
    //                 })
    //                 .collect::<Vec<_>>()
    //         })
    //         .collect();
        // }
     Ok(())
     
    }

