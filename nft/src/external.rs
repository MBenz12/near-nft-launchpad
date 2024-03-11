use crate::*;

//initiate a cross contract call to the nft contract. This will transfer the token to the buyer and return
//a payout object used for the market to distribute funds to the appropriate accounts.
#[ext_contract(ext_ft_contract)]
trait ExtFtContract {
    fn ft_transfer(
        &mut self,
        receiver_id: AccountId, 
        amount: U128, 
        memo: Option<String>
    );
}