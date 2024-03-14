// Find all our documentation at https://docs.near.org
use near_sdk::borsh::{BorshDeserialize, BorshSerialize};
use near_sdk::{
    near_bindgen, AccountId, env, Promise, NearToken, Gas,
    serde_json::json, log
};
use near_sdk::json_types::U128;
use near_contract_standards::non_fungible_token::metadata::{
    NFTContractMetadata
};
use near_sdk::serde::Serialize;

const NEAR_PER_STORAGE: u128 = 10_000_000_000_000_000_000;
const NFT_CONTRACT_STORAGE: u128 = 20_000_000_000_000_000_000_000;

// Define the contract structure
#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
#[borsh(crate = "near_sdk::borsh")]
pub struct Contract {}

impl Default for Contract {
    fn default() -> Self {
        Self {}
    }
}

// Implement the contract structure
#[near_bindgen]
impl Contract {
    #[payable]
    pub fn launch(
        &mut self,
        metadata: NFTContractMetadata,
        mint_price: U128,
        mint_currency: Option<AccountId>,
        payment_split_percent: U128,
    ) {
        let current_id = env::current_account_id();
        let owner = env::predecessor_account_id(); 

        let code = include_bytes!("./nft/nft.wasm").to_vec();
        let contract_bytes = code.len() as u128;
        let minimum_needed = NEAR_PER_STORAGE * contract_bytes + NFT_CONTRACT_STORAGE;

        // Deploy the nft contract
        let nft_contract_id: AccountId = format!("{}.{}", metadata.symbol.to_lowercase(), current_id).parse().unwrap();

        Promise::new(nft_contract_id.clone())
            .create_account()
            .transfer(NearToken::from_yoctonear(minimum_needed))
            .deploy_contract(code)
            .function_call(
                "new".to_string(),
                if let Some(mint_currency) = mint_currency.clone() {
                    json!({
                        "owner_id": owner.to_string(),
                        "metadata": metadata,
                        "mint_price": mint_price.0.to_string(),
                        "mint_currency": mint_currency.to_string(),
                        "payment_split_percent": payment_split_percent.0.to_string(),
                    })
                } else {
                    json!({
                        "owner_id": owner.to_string(),
                        "metadata": metadata,
                        "mint_price": mint_price.0.to_string(),
                        "payment_split_percent": payment_split_percent.0.to_string(),
                    })
                }.to_string().into_bytes().to_vec(),
                NearToken::from_yoctonear(0),
                Gas::from_tgas(20)
            );
        
            Event::Launch {
                creator_id: &owner,
                collection_id: &nft_contract_id,
                mint_price: &mint_price,
                mint_currency: mint_currency.as_ref(),
                name: &metadata.name,
                symbol: &metadata.symbol,
            }
            .emit();
    }
}

#[derive(Serialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
#[serde(tag = "event", content = "data")]
#[serde(rename_all = "snake_case")]
pub enum Event<'a> {
    Launch {
        creator_id: &'a AccountId,
        collection_id: &'a AccountId,
        mint_price: &'a U128,
        name: &'a String,
        symbol: &'a String,
        #[serde(skip_serializing_if = "Option::is_none")]
        mint_currency: Option<&'a AccountId>,
    }
}

impl Event<'_> {
    pub fn emit(&self) {
        emit_event(&self);
    }
}

const EVENT_STANDARD: &str = "linear";
const EVENT_STANDARD_VERSION: &str = "1.0.0";

// Emit event that follows NEP-297 standard: https://nomicon.io/Standards/EventsFormat
// Arguments
// * `standard`: name of standard, e.g. nep171
// * `version`: e.g. 1.0.0
// * `event`: type of the event, e.g. nft_mint
// * `data`: associate event data. Strictly typed for each set {standard, version, event} inside corresponding NEP
pub(crate) fn emit_event<T: ?Sized + Serialize>(data: &T) {
    let result = json!(data);
    let event_json = json!({
        "standard": EVENT_STANDARD,
        "version": EVENT_STANDARD_VERSION,
        "event": result["event"],
        "data": [result["data"]]
    })
    .to_string();
    log!(format!("EVENT_JSON:{}", event_json));
}