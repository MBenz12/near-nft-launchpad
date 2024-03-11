// Find all our documentation at https://docs.near.org
use near_sdk::borsh::{BorshDeserialize, BorshSerialize};
use near_sdk::{env, NearToken, Gas, near_bindgen, AccountId, Promise, serde_json::json, require};
use near_sdk::json_types::U128;
// use near_contract_standards::fungible_token::core_impl::FungibleToken;

// Define the contract structure
#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
#[borsh(crate = "near_sdk::borsh")]
pub struct Contract {
    pub owner_contract: AccountId,
    pub ft_contract:  Option<AccountId>,
    pub amount: u128,
}

impl Default for Contract {
    fn default() -> Self {
        Self {
            owner_contract: env::predecessor_account_id(),
            ft_contract: None, // You need to specify the default value for ft_contract
            amount: 0, // You need to specify the default value for amount
        }
    }
}

// Implement the contract structure
#[near_bindgen]
impl Contract {
    #[init]
    pub fn init(ft_contract: Option<AccountId>) -> Self {
        Self {
            owner_contract: env::predecessor_account_id(),
            ft_contract,
            amount: 0,
        }
    }
    
    #[payable]
    pub fn deposit_near(
        &mut self
    ) {
        let attached_amount = env::attached_deposit();
        self.amount = attached_amount.as_yoctonear();
    }

    #[payable]
    pub fn withdraw(
        &mut self,      
        owner: AccountId,
    ) {
        assert_eq!(
            env::predecessor_account_id(),
            self.owner_contract,
            "Only the owner contract can withdraw"
        );
        
        if let Some(ft_contract) = &self.ft_contract {
            Promise::new(ft_contract.clone()).function_call(
                "ft_transfer".to_string(), 
                json!({
                    "receiver_id": owner.to_string(),
                    "amount": self.amount.to_string(),                    
                }).to_string().into_bytes().to_vec(),
                NearToken::from_yoctonear(1),
                Gas::from_tgas(20),
            );
            Promise::new(ft_contract.clone()).function_call(
                "storage_withdraw".to_string(),
                json!({}).to_string().into_bytes().to_vec(),
                NearToken::from_yoctonear(1),
                Gas::from_tgas(20),
            );
        } else {
            Promise::new(owner).transfer(NearToken::from_yoctonear(self.amount));
        }
        self.amount = 0;
    }
}


trait FungibleTokenReceiver {
    fn ft_on_transfer(
        &mut self,
        sender_id: AccountId,
        amount: U128,
    ) -> U128;
}

#[near_bindgen]
impl FungibleTokenReceiver for Contract {
    fn ft_on_transfer(
        &mut self,
        sender_id: AccountId,
        amount: U128,
    ) -> U128 {
        // get the contract ID which is the predecessor
        let ft_contract_id = env::predecessor_account_id();
        if let Some(mint_currency) = self.ft_contract.clone() {
            // Ensure only the specified FT can be used
            require!(
                ft_contract_id == mint_currency,
                "FT contract ID does not match"
            );

            //get the signer which is the person who initiated the transaction
            let signer_id = env::signer_account_id();

            //make sure that the signer isn't the predecessor. This is so that we're sure
            //this was called via a cross-contract call
            assert_ne!(
                ft_contract_id,
                signer_id,
                "ft_on_transfer should only be called via cross-contract call"
            );
            //make sure the owner ID is the signer. 
            // assert_eq!(
            //     sender_id,
            //     signer_id,
            //     "owner_id should be signer_id"
            // );

            self.amount = amount.0;
        }

        U128(0)
    }
}