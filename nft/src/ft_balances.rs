use crate::*;

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
        if let Some(mint_currency) = self.mint_currency.clone() {
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
            assert_eq!(
                sender_id,
                signer_id,
                "owner_id should be signer_id"
            );

            // Add the amount to the user's current balance
            let mut cur_bal = self.ft_deposits.get(&signer_id).unwrap_or(0);
            cur_bal += amount.0;
            self.ft_deposits.insert(&signer_id, &cur_bal);

        }

        U128(0)
    }
}