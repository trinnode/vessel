use near_sdk::collections::{LookupMap, UnorderedMap};
use near_sdk::json_types::U128;
use near_sdk::{
    env, ext_contract, near, require, AccountId, Gas, NearToken, PanicOnDefault, Promise,
};

const GAS_WALLET_INIT: Gas = Gas::from_tgas(30);
const GAS_CALLBACK: Gas = Gas::from_tgas(15);

#[derive(Clone)]
#[near(serializers = [borsh, json])]
pub struct VesselMetadata {
    pub title: String,
    pub description: String,
    pub media: Option<String>,
}

#[derive(Clone)]
#[near(serializers = [borsh, json])]
pub struct TokenView {
    pub token_id: String,
    pub owner_id: AccountId,
    pub subaccount_id: AccountId,
    pub metadata: VesselMetadata,
    pub listed: bool,
}

#[ext_contract(ext_wallet)]
#[allow(dead_code, clippy::new_ret_no_self, clippy::wrong_self_convention)]
trait Wallet {
    fn new(&mut self, nft_contract: AccountId, token_id: String, owner_id: AccountId);
    fn owner_changed(&mut self, new_owner: AccountId);
    fn set_listed(&mut self, listed: bool);
    fn set_verifier(&mut self, verifier: AccountId);
}

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct VesselNft {
    owner_id: AccountId,
    wallet_code: Vec<u8>,
    verifier: Option<AccountId>,
    wallet_initial_balance: NearToken,
    next_token_id: u64,
    owners: UnorderedMap<String, AccountId>,
    wallets: LookupMap<String, AccountId>,
    metadata: UnorderedMap<String, VesselMetadata>,
    listed: LookupMap<String, bool>,
    marketplace: Option<AccountId>,
}

#[near]
impl VesselNft {
    #[init]
    #[payable]
    pub fn new(
        owner_id: AccountId,
        wallet_code: Vec<u8>,
        verifier: Option<AccountId>,
        wallet_initial_balance: U128,
    ) -> Self {
        require!(!wallet_code.is_empty(), "wallet_code is required");
        Self {
            owner_id,
            wallet_code,
            verifier,
            wallet_initial_balance: NearToken::from_yoctonear(wallet_initial_balance.0),
            next_token_id: 0,
            owners: UnorderedMap::new(b"o"),
            wallets: LookupMap::new(b"w"),
            metadata: UnorderedMap::new(b"m"),
            listed: LookupMap::new(b"l"),
            marketplace: None,
        }
    }

    pub fn set_marketplace(&mut self, marketplace: AccountId) {
        self.assert_owner();
        require!(self.marketplace.is_none(), "Marketplace already configured");
        self.marketplace = Some(marketplace);
    }

    #[payable]
    pub fn mint_vessel(&mut self, metadata: VesselMetadata) -> Promise {
        let minter = env::predecessor_account_id();
        let token_id = format!("{:06}", self.next_token_id);
        self.next_token_id += 1;
        let subaccount_id: AccountId = format!("vessel-{}.{}", token_id, env::current_account_id())
            .parse()
            .expect("generated subaccount must be valid");
        self.owners.insert(&token_id, &minter);
        self.wallets.insert(&token_id, &subaccount_id);
        self.metadata.insert(&token_id, &metadata);
        self.listed.insert(&token_id, &false);
        let mut promise = Promise::new(subaccount_id.clone())
            .create_account()
            .transfer(self.wallet_initial_balance)
            .deploy_contract(self.wallet_code.clone())
            .function_call(
                "new".to_string(),
                near_sdk::serde_json::json!({
                    "nft_contract": env::current_account_id(),
                    "token_id": token_id,
                    "owner_id": minter,
                })
                .to_string()
                .into_bytes(),
                NearToken::from_yoctonear(0),
                GAS_WALLET_INIT,
            );
        if let Some(verifier) = self.verifier.clone() {
            promise = promise.function_call(
                "set_verifier".to_string(),
                near_sdk::serde_json::json!({"verifier": verifier})
                    .to_string()
                    .into_bytes(),
                NearToken::from_yoctonear(0),
                GAS_WALLET_INIT,
            );
        }
        promise.then(
            Self::ext(env::current_account_id())
                .with_static_gas(GAS_CALLBACK)
                .on_mint(token_id),
        )
    }

    #[private]
    #[allow(deprecated)]
    pub fn on_mint(&mut self, _token_id: String) -> bool {
        require!(
            env::promise_results_count() == 1,
            "wallet initialization missing"
        );
        matches!(
            env::promise_result(0),
            near_sdk::PromiseResult::Successful(_)
        )
    }

    #[payable]
    pub fn nft_transfer(
        &mut self,
        receiver_id: AccountId,
        token_id: String,
        _approval_id: Option<u64>,
        _memo: Option<String>,
    ) {
        require!(
            env::attached_deposit().as_yoctonear() == 1,
            "Requires exactly 1 yoctoNEAR"
        );
        self.transfer_internal(&env::predecessor_account_id(), &receiver_id, &token_id);
    }

    pub fn lock_for_sale(&mut self, token_id: String, seller_id: AccountId) {
        self.assert_marketplace();
        require!(
            self.owners.get(&token_id) == Some(seller_id),
            "Only the owner can list"
        );
        require!(
            !self.listed.get(&token_id).unwrap_or(false),
            "Already listed"
        );
        self.listed.insert(&token_id, &true);
        if let Some(wallet) = self.wallets.get(&token_id) {
            let _ = ext_wallet::ext(wallet.clone())
                .with_static_gas(GAS_WALLET_INIT)
                .set_listed(true);
        }
    }

    pub fn unlock_from_sale(&mut self, token_id: String) {
        self.assert_marketplace();
        self.listed.insert(&token_id, &false);
        if let Some(wallet) = self.wallets.get(&token_id) {
            let _ = ext_wallet::ext(wallet.clone())
                .with_static_gas(GAS_WALLET_INIT)
                .set_listed(false);
        }
    }

    pub fn transfer_from_marketplace(&mut self, receiver_id: AccountId, token_id: String) {
        self.assert_marketplace();
        require!(
            self.listed.get(&token_id).unwrap_or(false),
            "Vessel is not listed"
        );
        self.owners.insert(&token_id, &receiver_id);
        self.listed.insert(&token_id, &false);
        if let Some(wallet) = self.wallets.get(&token_id) {
            let _ = ext_wallet::ext(wallet.clone())
                .with_static_gas(GAS_WALLET_INIT)
                .set_listed(false);
            let _ = ext_wallet::ext(wallet)
                .with_static_gas(GAS_WALLET_INIT)
                .owner_changed(receiver_id);
        }
    }

    pub fn nft_token(&self, token_id: String) -> Option<TokenView> {
        let owner_id = self.owners.get(&token_id)?;
        Some(TokenView {
            token_id: token_id.clone(),
            owner_id,
            subaccount_id: self.wallets.get(&token_id)?,
            metadata: self.metadata.get(&token_id)?,
            listed: self.listed.get(&token_id).unwrap_or(false),
        })
    }

    pub fn nft_total_supply(&self) -> U128 {
        U128(self.next_token_id as u128)
    }

    fn transfer_internal(&mut self, sender: &AccountId, receiver: &AccountId, token_id: &str) {
        let owner = self
            .owners
            .get(&token_id.to_string())
            .expect("Unknown vessel");
        require!(&owner == sender, "Not token owner");
        require!(
            !self.listed.get(&token_id.to_string()).unwrap_or(false),
            "Vessel is listed"
        );
        self.owners.insert(&token_id.to_string(), receiver);
        if let Some(wallet) = self.wallets.get(&token_id.to_string()) {
            let _ = ext_wallet::ext(wallet)
                .with_static_gas(GAS_WALLET_INIT)
                .owner_changed(receiver.clone());
        }
    }

    fn assert_owner(&self) {
        require!(
            env::predecessor_account_id() == self.owner_id,
            "Only contract owner"
        );
    }
    fn assert_marketplace(&self) {
        require!(
            self.marketplace.as_ref() == Some(&env::predecessor_account_id()),
            "Only marketplace"
        );
    }
}
