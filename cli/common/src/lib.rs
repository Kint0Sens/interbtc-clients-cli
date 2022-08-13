mod error;
pub use error::Error;
use sha2::{Digest, Sha256};
// use serde::{
    // Deserialize, 
    // Serialize,
// };
use secp256k1::{constants::PUBLIC_KEY_SIZE, Error as Secp256k1Error, PublicKey as Secp256k1PublicKey};use std::env;
use pad::{PadStr, Alignment};
use bitcoin_hashes::{hash160::Hash as Hash160, Hash};
// use clap::Parser;
// use std::str::FromStr;
// use std::ops::RangeInclusive;
use primitives::TokenSymbol;
use std::{
    thread,
    time::Duration,
    path::Path,
    ffi::OsStr,
    ops::RangeInclusive,
    str::FromStr,
};
use bitcoincore_rpc::{
    json::AddressType,
    RpcApi,
};
use bitcoin::{
    BitcoinCore,
    Network,
    // PublicKey as BtcPublicKey,
    // Address as BtcAddress,
};
// use bdk::{
//         Error as BdkError,
//         Wallet,
//         database::MemoryDatabase,
//         blockchain::ElectrumBlockchain,
//         electrum_client::Client,
//         };
use runtime::{
    H160,
    CurrencyId,
    CurrencyIdExt,
    CurrencyInfo,
    H256,
    InterBtcParachain,
    CollateralBalancesPallet,
    UtilFuncs,
    // AccountId,
//            InterBtcVault,
//         RedeemPallet,
//         InterBtcSigner,
//         BtcAddress,
    // VaultId,
//         Token,
//         KBTC,
//         parse_collateral_currency,
//         parse_wrapped_currency,
};

cfg_if::cfg_if! {
    if #[cfg(feature = "standalone-metadata")] {
        pub const BITCOIN_NETWORK : bitcoin::Network = Network::Testnet;
        pub const ELECTRUM : &str = "ssl://electrum.blockstream.info:60002";
        pub const TEXT_CONNECT_ATTEMPT : &str = "Attempting connection to Interlay Standalone parachain";
        pub const TEXT_CONNECTED : &str = "Connected to Interlay Standalone parachain";
        pub const TEXT_BTC_CONNECT_ATTEMPT : &str = "Attempting connection to bitcoin Testnet network"; // Or Regtest?
        pub const TEXT_BTC_CONNECTED : &str = "Connected to bitcoin Testnet network";
        pub const TEXT_BTC_BOT_WALLET : &str = "PremiumBotWallet-Standaone";
    } else if #[cfg(feature = "parachain-metadata-kintsugi")] {
        pub const BITCOIN_NETWORK : bitcoin::Network = Network::Bitcoin;
        pub const ELECTRUM : &str = "ssl://electrum.blockstream.info:50002";
        pub const TEXT_CONNECT_ATTEMPT : &str = "Attempting connection to Kintsugi parachain";
        pub const TEXT_CONNECTED : &str = "Connected to Kintsugi parachain";
        pub const TEXT_BTC_CONNECT_ATTEMPT : &str = "Attempting connection bitcoin Bitcoin network"; 
        pub const TEXT_BTC_CONNECTED : &str = "Connected to bitcoin Bitcoin network";
        pub const TEXT_BTC_BOT_WALLET : &str = "PremiumBotWallet-Kintsugi";
    } else if #[cfg(feature = "parachain-metadata-interlay")] {
        pub const BITCOIN_NETWORK : bitcoin::Network = Network::Bitcoin;
        pub const ELECTRUM : &str = "ssl://electrum.blockstream.info:50002";
        pub const TEXT_CONNECT_ATTEMPT : &str = "Attempting connection to Interlay parachain";
        pub const TEXT_CONNECTED : &str = "Connected to Interlay parachain";
        pub const TEXT_BTC_CONNECT_ATTEMPT : &str = "Attempting connection bitcoin Bitcoin network"; 
        pub const TEXT_BTC_CONNECTED : &str = "Connected to bitcoin Bitcoin network";
        pub const TEXT_BTC_BOT_WALLET : &str = "PremiumBotWallet-Interlay";
    } else if #[cfg(feature = "parachain-metadata-kintsugi-testnet")] {
        pub const BITCOIN_NETWORK : bitcoin::Network = Network::Testnet;
        pub const ELECTRUM : &str = "ssl://electrum.blockstream.info:60002";
        pub const TEXT_CONNECT_ATTEMPT : &str = "Attempting connection to Kintsugi Testnet parachain";
        pub const TEXT_CONNECTED : &str = "Connected to Kintsugi Testnet parachain";
        pub const TEXT_BTC_CONNECT_ATTEMPT : &str = "Attempting connection to bitcoin Testnet network";
        pub const TEXT_BTC_CONNECTED : &str = "Connected to bitcoin Testnet network";
        pub const TEXT_BTC_BOT_WALLET : &str = "PremiumBotWallet-Kintsugi-Testnet";
    } else if #[cfg(feature = "parachain-metadata-interlay-testnet")] {
        pub const BITCOIN_NETWORK : bitcoin::Network = Network::Testnet;
        pub const ELECTRUM : &str = "ssl://electrum.blockstream.info:60002";
        pub const TEXT_CONNECT_ATTEMPT : &str = "Attempting connection to Interlay Testnet parachain";
        pub const TEXT_CONNECTED : &str = "Connected to Interlay Testnet parachain";
        pub const TEXT_BTC_CONNECT_ATTEMPT : &str = "Attempting connection to bitcoin Testnet network";
        pub const TEXT_BTC_CONNECTED : &str = "Connected to bitcoin Testnet network";
        pub const TEXT_BTC_BOT_WALLET : &str = "PremiumBotWallet-Interlay-Testnet";
    }
}

pub const TEXT_SEPARATOR : &str = "-------------------------------";
pub const TEXT_BTC_WALLET_CONNECTED : &str = "BTC wallet connectd";

pub const SHORT_SLEEP: u64 = 2;

pub fn get_currency_str(token_symbol : TokenSymbol) -> String {
    match token_symbol {
        TokenSymbol::KINT => { "KINT".to_string() },
        TokenSymbol::DOT => { "DOT".to_string() },
        TokenSymbol::IBTC => { "IBTC".to_string() },
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

// pub fn setup_wallet(ext: String, int: String) -> Result<Wallet<ElectrumBlockchain, MemoryDatabase>, BdkError> {
//     let external_descriptor = &ext;
//     let internal_descriptor = &int;
//     Wallet::new(
//         external_descriptor,
//         Some(internal_descriptor),
//         BITCOIN_NETWORK,
//         MemoryDatabase::new(),
//         ElectrumBlockchain::from(Client::new(ELECTRUM).unwrap()),
//     )
// }

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
    pretty_print_planck_amount(amount, currency.inner().unwrap().decimals() as u32 )
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
            "Amount in sat should exceed {}",
            TOO_FEW_SATS.end()
        )),
    })
}

/// A Bitcoin address is a serialized identifier that represents the destination for a payment.
/// Address prefixes are used to indicate the network as well as the format. Since the Parachain
/// follows SPV assumptions we do not need to know which network a payment is included in.
#[derive(Clone, Ord, PartialOrd, PartialEq, Eq, Debug, Copy)]
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize, std::hash::Hash))]
pub enum Address {
    // input: {signature} {pubkey}
    // output: OP_DUP OP_HASH160 {hash160(pubkey)} OP_EQUALVERIFY OP_CHECKSIG
    // witness: <>
    P2PKH(H160),
    // input: [redeem_script_sig ...] {redeem_script}
    // output: OP_HASH160 {hash160(redeem_script)} OP_EQUAL
    // witness: <?>
    P2SH(H160),
    // input: <>
    // output: OP_0 {hash160(pubkey)}
    // witness: {signature} {pubkey}
    P2WPKHv0(H160),
    // input: <>
    // output: OP_0 {sha256(redeem_script)}
    // witness: [redeem_script_sig ...] {redeem_script}
    P2WSHv0(H256),
}

impl Address {
//     pub fn from_script_pub_key(script: &Script) -> Result<Self, Error> {
//         const OP_DUP: u8 = OpCode::OpDup as u8;
//         const OP_HASH_160: u8 = OpCode::OpHash160 as u8;
//         const OP_EQUAL_VERIFY: u8 = OpCode::OpEqualVerify as u8;
//         const OP_CHECK_SIG: u8 = OpCode::OpCheckSig as u8;
//         const OP_EQUAL: u8 = OpCode::OpEqual as u8;
//         const OP_0: u8 = OpCode::Op0 as u8;

//         match script.as_bytes() {
//             &[OP_DUP, OP_HASH_160, HASH160_SIZE_HEX, ref addr @ .., OP_EQUAL_VERIFY, OP_CHECK_SIG]
//                 if addr.len() == HASH160_SIZE_HEX as usize =>
//             {
//                 Ok(Self::P2PKH(H160::from_slice(addr)))
//             }
//             &[OP_HASH_160, HASH160_SIZE_HEX, ref addr @ .., OP_EQUAL] if addr.len() == HASH160_SIZE_HEX as usize => {
//                 Ok(Self::P2SH(H160::from_slice(addr)))
//             }
//             &[OP_0, HASH256_SIZE_HEX, ref addr @ ..] if addr.len() == HASH256_SIZE_HEX as usize => {
//                 Ok(Self::P2WSHv0(H256::from_slice(addr)))
//             }
//             &[OP_0, HASH160_SIZE_HEX, ref addr @ ..] if addr.len() == HASH160_SIZE_HEX as usize => {
//                 Ok(Self::P2WPKHv0(H160::from_slice(addr)))
//             }
//             _ => Err(Error::InvalidBtcAddress),
//         }
//     }

//     pub fn to_script_pub_key(&self) -> Script {
//         match self {
//             Self::P2PKH(pub_key_hash) => {
//                 let mut script = Script::new();
//                 script.append(OpCode::OpDup);
//                 script.append(OpCode::OpHash160);
//                 script.append(HASH160_SIZE_HEX);
//                 script.append(pub_key_hash);
//                 script.append(OpCode::OpEqualVerify);
//                 script.append(OpCode::OpCheckSig);
//                 script
//             }
//             Self::P2SH(script_hash) => {
//                 let mut script = Script::new();
//                 script.append(OpCode::OpHash160);
//                 script.append(HASH160_SIZE_HEX);
//                 script.append(script_hash);
//                 script.append(OpCode::OpEqual);
//                 script
//             }
//             Self::P2WPKHv0(pub_key_hash) => {
//                 let mut script = Script::new();
//                 script.append(OpCode::Op0);
//                 script.append(HASH160_SIZE_HEX);
//                 script.append(pub_key_hash);
//                 script
//             }
//             Self::P2WSHv0(script_hash) => {
//                 let mut script = Script::new();
//                 script.append(OpCode::Op0);
//                 script.append(HASH256_SIZE_HEX);
//                 script.append(script_hash);
//                 script
//             }
//         }
//     }

    pub fn random() -> Self {
        Address::P2PKH(H160::random())
    }
}

impl Default for Address {
    fn default() -> Self {
        Self::P2PKH(H160::zero())
    }
}

// #[derive(Clone, PartialEq, Eq, Debug)]
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PublicKey(pub [u8; PUBLIC_KEY_SIZE]);

impl Default for PublicKey {
    fn default() -> Self {
        Self([0; PUBLIC_KEY_SIZE])
    }
}

impl From<[u8; PUBLIC_KEY_SIZE]> for PublicKey {
    fn from(bytes: [u8; PUBLIC_KEY_SIZE]) -> Self {
        Self(bytes)
    }
}

impl serde::Serialize for PublicKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut slice = [0u8; 2 + 2 * PUBLIC_KEY_SIZE];
        impl_serde::serialize::serialize_raw(&mut slice, &self.0, serializer)
    }
}

// impl<'de> serde::Deserialize<'de> for PublicKey {
//     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
//     where
//         D: serde::Deserializer<'de>,
//     {
//         let mut bytes = [0u8; PUBLIC_KEY_SIZE];
//         impl_serde::serialize::deserialize_check_len(
//             deserializer,
//             impl_serde::serialize::ExpectedLen::Exact(&mut bytes),
//         )?;
//         Ok(PublicKey(bytes))
//     }
// }
pub mod global {
    use secp256k1::{ffi::types::AlignedType, AllPreallocated, Secp256k1};
    use sp_std::{ops::Deref, vec, vec::Vec};
    // this is what lazy_static uses internally
    use spin::Once;

    pub struct GlobalContext {
        __private: (),
    }

    pub static SECP256K1: &GlobalContext = &GlobalContext { __private: () };

    impl Deref for GlobalContext {
        type Target = Secp256k1<AllPreallocated<'static>>;

        fn deref(&self) -> &Self::Target {
            static ONCE: Once<()> = Once::new();
            static mut BUFFER: Vec<AlignedType> = vec![];
            static mut CONTEXT: Option<Secp256k1<AllPreallocated<'static>>> = None;
            ONCE.call_once(|| unsafe {
                BUFFER = vec![AlignedType::zeroed(); Secp256k1::preallocate_size()];
                let ctx = Secp256k1::preallocated_new(&mut BUFFER).unwrap();
                CONTEXT = Some(ctx);
            });
            unsafe { CONTEXT.as_ref().unwrap() }
        }
    }
}

impl PublicKey {
    fn new_secret_key(&self, secure_id: H256) -> [u8; 32] {
        let mut hasher = Sha256::default();
        // input compressed public key
        hasher.input(&self.0);
        // input secure id
        hasher.input(secure_id.as_bytes());

        let mut bytes = [0; 32];
        bytes.copy_from_slice(&hasher.result()[..]);
        bytes
    }
    //    / Generates an ephemeral "deposit" public key which can be used in Issue
    //     / requests to ensure that payments are unique.
    //     /
    //     / # Arguments
    //     /
    //     / * `secure_id` - random nonce (as provided by the security module)
    pub fn new_deposit_public_key(&self, secure_id: H256) -> Result<Self, Secp256k1Error> {
        self.new_deposit_public_key_with_secret(&self.new_secret_key(secure_id))
    }

    fn new_deposit_public_key_with_secret(&self, secret_key: &[u8; 32]) -> Result<Self, Secp256k1Error> {
        let mut public_key = Secp256k1PublicKey::from_slice(&self.0)?;
        // D = V * c
        public_key.mul_assign(global::SECP256K1, secret_key)?;
        Ok(Self(public_key.serialize()))
    }
}


fn new_deposit_public_key(wallet_public_key : PublicKey, secure_id: H256) -> Result<PublicKey, Error> {
    let deposit_public_key = wallet_public_key
        .new_deposit_public_key(secure_id)
        .map_err(|_| Error::InvalidPublicKey)?;
    Ok(deposit_public_key)
}


pub fn new_deposit_address(wallet_public_key : PublicKey, secure_id: H256) -> Result<Address, Error> {
    let public_key = new_deposit_public_key(wallet_public_key, secure_id)?;
     // let btc_address = BtcAddress::P2WPKHv0(public_key.to_hash());
    let btc_address =H160::from(Hash160::hash(&public_key.0).into_inner());
    Ok(Address::P2WPKHv0(btc_address))
}

pub fn get_new_btc_address(bitcoin_core: BitcoinCore) -> Result<bitcoin::Address, Error> {
    let address = bitcoin_core.rpc.get_new_address(None,Some(AddressType::Bech32))?;
    Ok(address)
}

 // Sleep 'duration' seconds but insert a ping to parachain every 30 seconds to keep connectin open
pub async fn sleep_with_parachain_ping(parachain: InterBtcParachain, sleep_seconds: u64) -> Result<(), Error> {
    let sleep_duration =  Duration::from_secs(sleep_seconds);
    let keepalive_seconds : u64 =  15;
    let keepalive_duration = Duration::from_secs(keepalive_seconds);
    let mut elapsed_seconds : u64 = 0;
    loop {
        
        if elapsed_seconds == 0 {
            if sleep_seconds < keepalive_seconds {
                thread::sleep(sleep_duration);
            } else {
                thread::sleep(keepalive_duration);
            };   
        } else {
            thread::sleep(keepalive_duration);
        }
        // action to keep connection alive
        let native_id = parachain.get_native_currency_id();
        let _free_balance = parachain.get_free_balance(native_id.clone()).await?;
        elapsed_seconds = elapsed_seconds + keepalive_seconds;
        if elapsed_seconds >= sleep_seconds { break };
    };
    Ok(())
}   
