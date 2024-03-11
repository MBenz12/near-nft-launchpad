// Find all our documentation at https://docs.near.org
use near_sdk::borsh::{BorshDeserialize, BorshSerialize};
use near_sdk::{
    near_bindgen, AccountId, env, Promise, NearToken, Gas,
    serde_json::json,
};
use near_sdk::json_types::U128;
use near_contract_standards::non_fungible_token::metadata::{
    NFTContractMetadata
};

const NEAR_PER_STORAGE: u128 = 10_000_000_000_000_000_000;
const NFT_CONTRACT_STORAGE: u128 = 20_000_000_000_000_000_000_000;

// Define the contract structure
#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
#[borsh(crate = "near_sdk::borsh")]
pub struct Contract {
    
}

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
    }
}
