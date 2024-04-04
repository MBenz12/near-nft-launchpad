/*!
Non-Fungible Token implementation with JSON serialization.
NOTES:
  - The maximum balance value is limited by U128 (2**128 - 1).
  - JSON calls should pass U128 as a base-10 string. E.g. "100".
  - The contract optimizes the inner trie structure by hashing account IDs. It will prevent some
    abuse of deep tries. Shouldn't be an issue, once NEAR clients implement full hashing of keys.
  - The contract tracks the change in storage before and after the call. If the storage increases,
    the contract requires the caller of the contract to attach enough deposit to the function call
    to cover the storage cost.
    This is done to prevent a denial of service attack on the contract by taking all available storage.
    If the storage decreases, the contract will issue a refund for the cost of the released storage.
    The unused tokens from the attached deposit are also refunded, so it's safe to
    attach more deposit than required.
  - To prevent the deployed contract from being modified or deleted, it should not have any access
    keys on its account.
*/
use near_contract_standards::non_fungible_token::approval::NonFungibleTokenApproval;
use near_contract_standards::non_fungible_token::core::{
    NonFungibleTokenCore, NonFungibleTokenResolver,
};
use near_contract_standards::non_fungible_token::enumeration::NonFungibleTokenEnumeration;
use near_contract_standards::non_fungible_token::metadata::{
    NFTContractMetadata, NonFungibleTokenMetadataProvider, TokenMetadata,
};
use near_contract_standards::non_fungible_token::NonFungibleToken;
use near_contract_standards::non_fungible_token::events::NftMint;
use near_contract_standards::non_fungible_token::{Token, TokenId};
use near_contract_standards::fungible_token::Balance;
use near_sdk::borsh::{BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LazyOption, LookupMap};
use near_sdk::json_types::U128;
use near_sdk::{
    env, near_bindgen, require, AccountId, BorshStorageKey, PanicOnDefault, Promise, PromiseOrValue, NearToken, Gas, 
    serde_json::json,
};
use std::collections::HashMap;

mod ft_balances;

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
#[borsh(crate = "near_sdk::borsh")]
pub struct Contract {
    pub tokens: NonFungibleToken,

    pub metadata: LazyOption<NFTContractMetadata>,

    pub index: u128,

    pub total_supply: u128,

    pub mint_price: u128,
    
    //which fungible token can be used to purchase NFTs
    pub mint_currency: Option<AccountId>, 
    
    pub payment_split_percent: u8,

    //keep track of the storage that accounts have payed
    pub storage_deposits: LookupMap<AccountId, u128>,

    //keep track of how many FTs each account has deposited in order to purchase NFTs with
    pub ft_deposits: LookupMap<AccountId, Balance>,
}

const NEAR_PER_STORAGE: u128 = 10_000_000_000_000_000_000;
//the minimum storage to have a sale on the contract.
const STORAGE_PER_SALE: u128 = 1000 * NEAR_PER_STORAGE;
const VAULT_STORAGE: u128 = 20_000_000_000_000_000_000_000;

#[derive(BorshSerialize, BorshStorageKey)]
#[borsh(crate = "near_sdk::borsh")]
enum StorageKey {
    NonFungibleToken,
    Metadata,
    TokenMetadata,
    Enumeration,
    Approval,
    StorageDeposits,
    FTDeposits,
}

#[near_bindgen]
impl Contract {
    /// Initializes the contract owned by `owner_id` with
    /// default metadata (for example purposes only).
    // #[init]
    // pub fn new_default_meta(owner_id: AccountId) -> Self {
    //     Self::new(
    //         owner_id,
    //         NFTContractMetadata {
    //             spec: NFT_METADATA_SPEC.to_string(),
    //             name: "Example NEAR non-fungible token".to_string(),
    //             symbol: "EXAMPLE".to_string(),
    //             icon: Some(DATA_IMAGE_SVG_NEAR_ICON.to_string()),
    //             base_uri: None,
    //             reference: None,
    //             reference_hash: None,
    //         },
    //         U128(0),
    //         None,
    //         U128(0),
    //         U128(0),
    //     )
    // }

    #[init]
    pub fn new(
        owner_id: AccountId, 
        metadata: NFTContractMetadata,
        mint_price: U128,
        mint_currency: Option<AccountId>,
        payment_split_percent: U128,
        total_supply: U128,
    ) -> Self {
        require!(!env::state_exists(), "Already initialized");
        metadata.assert_valid();
        Self {
            tokens: NonFungibleToken::new(
                StorageKey::NonFungibleToken,
                owner_id,
                Some(StorageKey::TokenMetadata),
                Some(StorageKey::Enumeration),
                Some(StorageKey::Approval),
            ),
            metadata: LazyOption::new(StorageKey::Metadata, Some(&metadata)),
            index: 0,
            total_supply: total_supply.0,
            mint_price: mint_price.0,
            mint_currency,
            payment_split_percent: payment_split_percent.0 as u8,
            storage_deposits: LookupMap::new(StorageKey::StorageDeposits),
            ft_deposits: LookupMap::new(StorageKey::FTDeposits),
        }
    }

    /// Mint a new token with ID=`token_id` belonging to `token_owner_id`.
    ///
    /// Since this example implements metadata, it also requires per-token metadata to be provided
    /// in this call. `self.tokens.mint` will also require it to be Some, since
    /// `StorageKey::TokenMetadata` was provided at initialization.
    ///
    /// `self.tokens.mint` will enforce `predecessor_account_id` to equal the `owner_id` given in
    /// initialization call to `new`.
    #[payable]
    pub fn nft_mint(
        &mut self,
        token_id: TokenId,
        token_owner_id: AccountId,
        token_metadata: TokenMetadata,
    ) -> Token {
        let collection_owner = &self.tokens.owner_id;
        let owner = env::predecessor_account_id(); 
        // assert_eq!(owner, self.tokens.owner_id, "Unauthorized");

        let code = include_bytes!("./vault/vault.wasm").to_vec();
        let contract_bytes = code.len() as u128;
        let minimum_needed = NEAR_PER_STORAGE * contract_bytes + VAULT_STORAGE;

        let deposit: u128 = env::attached_deposit().as_yoctonear();
        if let Some(_) = self.mint_currency.clone() {
            let amount = self.ft_deposits_of(owner.clone());
            require!(deposit >= minimum_needed && amount >= self.mint_price, "Insufficient price to mint");
        } else {
            require!(deposit >= self.mint_price + minimum_needed, "Insufficient price to mint");
        }

        let current_id = env::current_account_id();

        let vault_amount = self.mint_price.checked_mul(self.payment_split_percent.into())
            .unwrap().checked_div(100u128).unwrap();

        let owner_amount = self.mint_price.checked_sub(vault_amount).unwrap();

        // Deploy the vault contract
        let vault_account_id: AccountId = format!("{}.{}", token_id, current_id).parse().unwrap();
        Promise::new(vault_account_id.clone())
            .create_account()
            .deploy_contract(code)
            .transfer(NearToken::from_yoctonear(minimum_needed))
            .function_call(
                // Init the vault contract
                "init".to_string(),
                if let Some(ft_id) = self.mint_currency.clone() {
                    json!({
                        "ft_contract": ft_id.to_string()
                    })
                } else {
                    json!({})
                }.to_string().into_bytes().to_vec(),
                NearToken::from_millinear(0),
                Gas::from_tgas(20)
            )
            .then(
                // Deposit ft or near
                if let Some(ft_id) = self.mint_currency.clone() {
                    Promise::new(ft_id.clone()).function_call(
                        "storage_deposit".to_string(), 
                        json!({
                            "account_id": vault_account_id.to_string()
                        }).to_string().into_bytes().to_vec(),
                        NearToken::from_millinear(30),
                        Gas::from_tgas(20),
                    );

                    Promise::new(ft_id.clone()).function_call(
                        "ft_transfer_call".to_string(), 
                        json!({
                            "receiver_id": vault_account_id.to_string(),
                            "amount": vault_amount.to_string(),
                            "msg": "",
                        }).to_string().into_bytes().to_vec(),
                        NearToken::from_yoctonear(1),
                        Gas::from_tgas(50),
                    );

                    Promise::new(ft_id.clone()).function_call(
                        "storage_deposit".to_string(), 
                        json!({
                            "account_id": collection_owner.clone().to_string()
                        }).to_string().into_bytes().to_vec(),
                        NearToken::from_millinear(30),
                        Gas::from_tgas(20),
                    );

                    Promise::new(ft_id.clone()).function_call(
                        "ft_transfer".to_string(), 
                        json!({
                            "receiver_id": collection_owner.clone().to_string(),
                            "amount": owner_amount.to_string(),
                            "msg": "",
                        }).to_string().into_bytes().to_vec(),
                        NearToken::from_yoctonear(1),
                        Gas::from_tgas(50),
                    )
                } else {
                    Promise::new(collection_owner.clone()).transfer(NearToken::from_yoctonear(owner_amount));
                    Promise::new(vault_account_id.clone()).function_call(
                        "deposit_near".to_string(),
                        json!({}).to_string().into_bytes().to_vec(),
                        NearToken::from_yoctonear(vault_amount),
                        Gas::from_tgas(20),
                    )
                }
            );

        self.index = self.index.checked_add(1).unwrap();
        if self.total_supply > 0 {
            require!(self.total_supply >= self.index, "Exceeded total supply");
        }

        let token = self.tokens.internal_mint_with_refund(token_id, token_owner_id, Some(token_metadata), None);
        NftMint { owner_id: &token.owner_id, token_ids: &[&token.token_id], memo: None }.emit();
        token
    }

    //Allows users to deposit storage. This is to cover the cost of storing sale objects on the contract
    //Optional account ID is to users can pay for storage for other people.
    #[payable]
    pub fn storage_deposit(&mut self, account_id: Option<AccountId>) {
        //get the account ID to pay for storage for
        let storage_account_id = account_id 
            //convert the valid account ID into an account ID
            .map(|a| a.into())
            //if we didn't specify an account ID, we simply use the caller of the function
            .unwrap_or_else(env::predecessor_account_id);

        //get the deposit value which is how much the user wants to add to their storage
        let deposit: u128 = env::attached_deposit().as_yoctonear();

        //make sure the deposit is greater than or equal to the minimum storage for a sale
        assert!(
            deposit >= STORAGE_PER_SALE,
            "Requires minimum deposit of {}",
            STORAGE_PER_SALE
        );

        //get the balance of the account (if the account isn't in the map we default to a balance of 0)
        let mut balance: u128 = self.storage_deposits.get(&storage_account_id).unwrap_or(0);
        //add the deposit to their balance
        balance += deposit;
        //insert the balance back into the map for that account ID
        self.storage_deposits.insert(&storage_account_id, &balance);
    }

    // Burn an NFT by its token ID
    #[payable]
    pub fn burn(&mut self, token_id: TokenId) {
        let owner = env::predecessor_account_id();

        // Ensure the owner has the NFT
        assert!(self.tokens.owner_by_id.contains_key(&token_id), "You don't own this NFT");

        // Remove the NFT from the owner's account
        self.tokens.owner_by_id.remove(&token_id);

        // Remove token metadata (if applicable)
        self.tokens
            .token_metadata_by_id
            .as_mut()
            .and_then(|by_id| by_id.remove(&token_id));
        
        // Remove the NFT from the tokens_per_owner map
        if let Some(tokens_per_owner) = &mut self.tokens.tokens_per_owner {
            let mut owner_tokens = tokens_per_owner.get(&owner).unwrap_or_else(|| {
                env::panic_str("Unable to access tokens per owner in unguarded call.")
            });
            owner_tokens.remove(&token_id);
            if owner_tokens.is_empty() {
                tokens_per_owner.remove(&owner);
            } else {
                tokens_per_owner.insert(&owner, &owner_tokens);
            }
        }
        
        // Remove any approvals associated with this NFT
        self.tokens
            .approvals_by_id
            .as_mut()
            .and_then(|by_id| by_id.remove(&token_id.clone()));

        // Remove next approval ID (if applicable)
        self.tokens
            .next_approval_id_by_id
            .as_mut()
            .and_then(|by_id| by_id.remove(&token_id.clone()));

        let current_id = env::current_account_id();
        let vault_account_id: AccountId = format!("{}.{}", token_id, current_id).parse().unwrap();

        Promise::new(vault_account_id.clone()).function_call(
            "withdraw".to_string(),
            json!({
                "owner": owner.to_string()
            }).to_string().into_bytes().to_vec(),
            NearToken::from_yoctonear(1),
            Gas::from_tgas(100)
        );
    }

    //return how much storage an account has paid for
    pub fn storage_balance_of(&self, account_id: AccountId) -> U128 {
        U128(self.storage_deposits.get(&account_id).unwrap_or(0))
    }

    /// Get the amount of FTs the user has deposited into the contract
    pub fn ft_deposits_of(
        &self,
        account_id: AccountId
    ) -> u128 {
        self.ft_deposits.get(&account_id).unwrap_or(0)
    }

    pub fn index(&self) -> u128 {
        self.index
    }

    pub fn total_supply(&self) -> u128 {
        self.total_supply
    }
}

#[near_bindgen]
impl NonFungibleTokenCore for Contract {
    #[payable]
    fn nft_transfer(
        &mut self,
        receiver_id: AccountId,
        token_id: TokenId,
        approval_id: Option<u64>,
        memo: Option<String>,
    ) {
        self.tokens.nft_transfer(receiver_id, token_id, approval_id, memo);
    }

    #[payable]
    fn nft_transfer_call(
        &mut self,
        receiver_id: AccountId,
        token_id: TokenId,
        approval_id: Option<u64>,
        memo: Option<String>,
        msg: String,
    ) -> PromiseOrValue<bool> {
        self.tokens.nft_transfer_call(receiver_id, token_id, approval_id, memo, msg)
    }

    fn nft_token(&self, token_id: TokenId) -> Option<Token> {
        self.tokens.nft_token(token_id)
    }
}

#[near_bindgen]
impl NonFungibleTokenResolver for Contract {
    #[private]
    fn nft_resolve_transfer(
        &mut self,
        previous_owner_id: AccountId,
        receiver_id: AccountId,
        token_id: TokenId,
        approved_account_ids: Option<HashMap<AccountId, u64>>,
    ) -> bool {
        self.tokens.nft_resolve_transfer(
            previous_owner_id,
            receiver_id,
            token_id,
            approved_account_ids,
        )
    }
}

#[near_bindgen]
impl NonFungibleTokenApproval for Contract {
    #[payable]
    fn nft_approve(
        &mut self,
        token_id: TokenId,
        account_id: AccountId,
        msg: Option<String>,
    ) -> Option<Promise> {
        self.tokens.nft_approve(token_id, account_id, msg)
    }

    #[payable]
    fn nft_revoke(&mut self, token_id: TokenId, account_id: AccountId) {
        self.tokens.nft_revoke(token_id, account_id);
    }

    #[payable]
    fn nft_revoke_all(&mut self, token_id: TokenId) {
        self.tokens.nft_revoke_all(token_id);
    }

    fn nft_is_approved(
        &self,
        token_id: TokenId,
        approved_account_id: AccountId,
        approval_id: Option<u64>,
    ) -> bool {
        self.tokens.nft_is_approved(token_id, approved_account_id, approval_id)
    }
}

#[near_bindgen]
impl NonFungibleTokenEnumeration for Contract {
    fn nft_total_supply(&self) -> U128 {
        self.tokens.nft_total_supply()
    }

    fn nft_tokens(&self, from_index: Option<U128>, limit: Option<u64>) -> Vec<Token> {
        self.tokens.nft_tokens(from_index, limit)
    }

    fn nft_supply_for_owner(&self, account_id: AccountId) -> U128 {
        self.tokens.nft_supply_for_owner(account_id)
    }

    fn nft_tokens_for_owner(
        &self,
        account_id: AccountId,
        from_index: Option<U128>,
        limit: Option<u64>,
    ) -> Vec<Token> {
        self.tokens.nft_tokens_for_owner(account_id, from_index, limit)
    }
}

#[near_bindgen]
impl NonFungibleTokenMetadataProvider for Contract {
    fn nft_metadata(&self) -> NFTContractMetadata {
        self.metadata.get().unwrap()
    }
}
