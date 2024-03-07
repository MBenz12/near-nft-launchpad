// Find all our documentation at https://docs.near.org
use near_sdk::borsh::{BorshDeserialize, BorshSerialize};
use near_sdk::{env, NearToken, Gas, near_bindgen, AccountId, Promise, serde_json::json};


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
    
    pub fn deposit(
        &mut self,
        amount: u128
    ) {
        if let Some(ft_contract) = &self.ft_contract {
            self.amount = amount;
            Promise::new(ft_contract.clone()).function_call(
                "ft_transfer".to_string(), 
                json!({
                    "receiver_id": env::current_account_id().to_string(),
                    "amount": amount.to_string(),
                    "msg": "",
                }).to_string().into_bytes().to_vec(),
                NearToken::from_yoctonear(1),
                Gas::from_tgas(200),
            );
        } else {
            let amount = env::attached_deposit();
            self.amount = amount.as_yoctonear();
            Promise::new(env::current_account_id()).transfer(amount);
        }
    }

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
                    "msg": "",
                }).to_string().into_bytes().to_vec(),
                NearToken::from_yoctonear(1),
                Gas::from_tgas(200),
            );
        } else {
            Promise::new(owner).transfer(NearToken::from_yoctonear(self.amount));
        }
        self.amount = 0;
    }
}
