use near_sdk::collections::UnorderedMap;
use near_sdk::json_types::U128;
use near_sdk::{
    env, ext_contract, near, require, AccountId, Gas, NearToken, PanicOnDefault, Promise,
};

#[derive(Clone)]
#[near(serializers = [borsh, json])]
pub struct Listing {
    pub token_id: String,
    pub seller_id: AccountId,
    pub price: u128,
    pub listed_at: u64,
}

#[ext_contract(ext_nft)]
#[allow(dead_code)]
trait Nft {
    fn lock_for_sale(&mut self, token_id: String, seller_id: AccountId);
    fn unlock_from_sale(&mut self, token_id: String);
    fn transfer_from_marketplace(&mut self, receiver_id: AccountId, token_id: String);
}

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct VesselMarketplace {
    owner_id: AccountId,
    nft_contract: AccountId,
    fee_bps: u16,
    listings: UnorderedMap<String, Listing>,
}

#[near]
impl VesselMarketplace {
    #[init]
    pub fn new(owner_id: AccountId, nft_contract: AccountId, fee_bps: u16) -> Self {
        require!(fee_bps <= 1_000, "fee_bps must be at most 10%");
        Self {
            owner_id,
            nft_contract,
            fee_bps,
            listings: UnorderedMap::new(b"l"),
        }
    }

    #[payable]
    pub fn list(&mut self, token_id: String, price: U128) -> Promise {
        require!(price.0 > 0, "Price must be greater than zero");
        require!(self.listings.get(&token_id).is_none(), "Already listed");
        let seller_id = env::predecessor_account_id();
        self.listings.insert(
            &token_id,
            &Listing {
                token_id: token_id.clone(),
                seller_id: seller_id.clone(),
                price: price.0,
                listed_at: env::block_timestamp(),
            },
        );
        ext_nft::ext(self.nft_contract.clone())
            .with_static_gas(near_sdk::Gas::from_tgas(30))
            .lock_for_sale(token_id.clone(), seller_id)
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(Gas::from_tgas(10))
                    .on_lock(token_id),
            )
    }

    #[private]
    #[allow(deprecated)]
    pub fn on_lock(&mut self, token_id: String) -> bool {
        require!(env::promise_results_count() == 1, "lock result missing");
        if !matches!(
            env::promise_result(0),
            near_sdk::PromiseResult::Successful(_)
        ) {
            self.listings.remove(&token_id);
            return false;
        }
        true
    }

    pub fn unlist(&mut self, token_id: String) -> Promise {
        let listing = self.listings.get(&token_id).expect("Listing not found");
        require!(
            listing.seller_id == env::predecessor_account_id(),
            "Only seller can unlist"
        );
        self.listings.remove(&token_id);
        ext_nft::ext(self.nft_contract.clone())
            .with_static_gas(near_sdk::Gas::from_tgas(30))
            .unlock_from_sale(token_id)
    }

    #[payable]
    pub fn buy(&mut self, token_id: String) -> Promise {
        let listing = self.listings.get(&token_id).expect("Listing not found");
        require!(
            env::attached_deposit().as_yoctonear() == listing.price,
            "Attached deposit must equal listing price"
        );
        let buyer_id = env::predecessor_account_id();
        let fee = listing.price.saturating_mul(self.fee_bps as u128) / 10_000;
        self.listings.remove(&token_id);
        let _ = Promise::new(listing.seller_id.clone())
            .transfer(NearToken::from_yoctonear(listing.price - fee));
        ext_nft::ext(self.nft_contract.clone())
            .with_static_gas(near_sdk::Gas::from_tgas(30))
            .transfer_from_marketplace(buyer_id, token_id)
    }

    pub fn get_listing(&self, token_id: String) -> Option<Listing> {
        self.listings.get(&token_id)
    }
    pub fn get_fee_bps(&self) -> u16 {
        self.fee_bps
    }
    pub fn set_fee_bps(&mut self, fee_bps: u16) {
        require!(env::predecessor_account_id() == self.owner_id, "Only owner");
        require!(fee_bps <= 1_000, "fee_bps must be at most 10%");
        self.fee_bps = fee_bps;
    }
}
