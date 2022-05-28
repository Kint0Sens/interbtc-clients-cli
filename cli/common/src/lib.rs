use std::env;
mod error;
pub use error::Error;
use pad::{PadStr, Alignment};
// use clap::Parser;
// use std::str::FromStr;
// use std::ops::RangeInclusive;
use primitives::TokenSymbol;
use std::path::Path;
use std::ffi::OsStr;
use std::ops::RangeInclusive;
use std::str::FromStr;
use bitcoin::Network;
use bdk::{
        Error as BdkError,
        Wallet,
        database::MemoryDatabase,
        blockchain::ElectrumBlockchain,
        electrum_client::Client,
        };
//cli related
//interBTC related
use runtime::{
//            InterBtcVault,
//         RedeemPallet,
//         CollateralBalancesPallet,
//         InterBtcSigner,
//         UtilFuncs,
//         BtcAddress,
        // VaultId,
//         AccountId,
            CurrencyId,
            CurrencyIdExt,
            CurrencyInfo,
//         Token,
//         KBTC,
//         parse_collateral_currency,
//         parse_wrapped_currency,
        };

cfg_if::cfg_if! {
    if #[cfg(feature = "standalone-metadata")] {
        pub const BITCOIN_NETWORK : bitcoin::Network = Network::Testnet;
        pub const ELECTRUM : &str = "ssl://electrum.blockstream.info:60002";
        pub const TEXT_CONNECT_ATTEMPT : &str = "Attempting connection to Standalone parachain";
        pub const TEXT_CONNECTED : &str = "Connected to Standalone parachain";
            } else if #[cfg(feature = "parachain-metadata-kintsugi")] {
        pub const BITCOIN_NETWORK : bitcoin::Network = Network::Bitcoin;
        pub const ELECTRUM : &str = "ssl://electrum.blockstream.info:50002";
        pub const TEXT_CONNECT_ATTEMPT : &str = "Attempting connection to Kintsugi parachain";
        pub const TEXT_CONNECTED : &str = "Connected to Kintsugi parachain";
    } else if #[cfg(feature = "parachain-metadata-testnet")] {
        pub const BITCOIN_NETWORK : bitcoin::Network = Network::Testnet;
        pub const ELECTRUM : &str = "ssl://electrum.blockstream.info:60002";
        pub const TEXT_CONNECT_ATTEMPT : &str = "Attempting connection to Testnet parachain";
        pub const TEXT_CONNECTED : &str = "Connected to Testnet parachain";
    }
}

pub const TEXT_SEPARATOR : &str = "-------------------------------";

pub fn get_currency_str(token_symbol : TokenSymbol) -> String {
    match token_symbol {
        TokenSymbol::KINT => { "KINT".to_string() },
        TokenSymbol::DOT => { "DOT".to_string() },
        TokenSymbol::IBTC => { "KINT".to_string() },
        TokenSymbol::INTR => { "INTR".to_string() },
        TokenSymbol::KBTC => { "KBTC".to_string() },
        TokenSymbol::KSM => { "KSM".to_string() },
    }
} 

// #[allow(unused_assignments)]
// pub fn native_currency() -> String {
//     let mut native_currency = "INTR".to_string();
//     cfg_if::cfg_if! {
//         if #[cfg(feature = "standalone-metadata")] {
//             native_currency =  "KINT".to_string();
//         } else if #[cfg(feature = "parachain-metadata-kintsugi")] {
//             native_currency =  "KINT".to_string();
//         } else if #[cfg(feature = "parachain-metadata-testnet")] {
//             native_currency =  "KINT".to_string();
//         }
//     }
//     native_currency
// }

// Get Program Name    
pub fn prog_name_get() -> Option<String> {
    env::args().next()
        .as_ref()
        .map(Path::new)
        .and_then(Path::file_name)
        .and_then(OsStr::to_str)
        .map(String::from)
}

// pub fn setup_logging( verbose: usize) {
//         env_logger::init_from_env(
//         if opts.verbose > 0 {
//             env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, log::LevelFilter::Info.as_str())
//         } else {
//             env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, prog_name_get().unwrap())
//         }
//     );

// }

pub fn init_logger(verbose : usize) -> env_logger::Env<'static> {
    if verbose > 0 {
        let log_filter_substring1 : String = format!("{}{}",log::LevelFilter::Info.to_string(),",");
        let log_filter_substring2 : String = format!("{}{}", prog_name_get().unwrap(),"=trace");
        let log_filter = format!("{}{}",log_filter_substring1,log_filter_substring2);
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, log_filter)
    } else {
        let log_filter : String = format!("{}{}", prog_name_get().unwrap(),"=info");
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, log_filter)
    }
}

// #[allow(unused_assignments)]
// pub fn get_btc_network() -> Network {
//     cfg_if::cfg_if! {
//         if #[cfg(feature = "standalone-metadata")] {
//             let network = Network::Testnet;
//         } else if #[cfg(feature = "parachain-metadata-testnet")] {
//             let network = Network::Testnet;
//         } else {
//             let network = Network::Bitcoin;
//         }    
//     }
//     network
// }

pub fn setup_wallet(ext: String, int: String) -> Result<Wallet<ElectrumBlockchain, MemoryDatabase>, BdkError> {
    let external_descriptor = &ext;
    let internal_descriptor = &int;
    Wallet::new(
        external_descriptor,
        Some(internal_descriptor),
        BITCOIN_NETWORK,
        MemoryDatabase::new(),
        ElectrumBlockchain::from(Client::new(ELECTRUM).unwrap()),
    )
}

//Formatting of long absolute amount, based on number of decimals
pub fn pretty_print_planck_amount(amount: u128, decimals: u32) -> Result<String, Error> {

    let decimal_scale = 10_u128.pow( decimals);
    let amount_units = amount
        .checked_div(decimal_scale)
        .ok_or(error::Error::MathError)?;
    let amount_decimals = amount
        .checked_rem(decimal_scale)
        .ok_or(error::Error::MathError)?;


    let amount_decimals_str = format!("{}",amount_decimals)
                                        .pad(decimals as usize,'0',Alignment::Right, true);
    //return str
    Ok(format!("{}.{}",amount_units,amount_decimals_str))
}

pub fn pretty_print_currency_amount(amount: u128, currency: CurrencyId) -> Result<String, Error> {
    pretty_print_planck_amount(amount, currency.inner().decimals() as u32 )
}


pub const TOO_FEW_SATS: RangeInclusive<u128> = 1..=1999;
pub fn amount_gt_minimal(s: &str) -> Result<(), String> {
    //TODO: Dynamic calc of minimal amount?
    u128::from_str(s)
    .map(|amt| !TOO_FEW_SATS.contains(&amt))
    .map_err(|e| e.to_string())
    .and_then(|result| match result {
        true => Ok(()),
        false => Err(format!(
            "Amount in Sat should exceed {}",
            TOO_FEW_SATS.end()
        )),
    })
}
