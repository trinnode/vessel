//! Per-vessel sub-account wallet.
//!
//! One instance is deployed by the NFT factory at mint time on a sub-account
//! like `vessel-000001.vessel.near`. It holds the portfolio: either direct
//! NEP-141 balances (tracked locally via ft_on_transfer) or intents holdings
//! that live in the NEAR Intents verifier (read live, not stored here).
//!
//! TRUST MODEL. This contract trusts:
//! - the factory (the nft_contract passed to init). The factory alone can
//!   change the recorded owner, toggle the listing freeze and set the
//!   verifier. The factory is trusted code deployed once by the project.
//! - the verifier for custody of intents assets. Balances in intents.near
//!   are not tracked here; withdrawal of those assets is a promise to the
//!   verifier, which enforces its own authorization.
//! It does NOT trust the marketplace directly: the marketplace never calls
//! this contract, it goes through the factory. The recorded owner is only a
//! mirror of the NFT owner pushed by the factory, so the wallet is
//! withdrawable by whoever holds the NFT and nobody else.
//!
//! The recorded owner is event-sourced: it starts as the minter and is
//! updated via owner_changed promises the factory sends after an NFT
//! transfer resolves. View calls cannot be made synchronously from a
//! mutating context, so the wallet cannot read the NFT owner at withdraw
//! time; the push model is the honest alternative.

use near_sdk::collections::UnorderedMap;
use near_sdk::json_types::U128;
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{
    env, ext_contract, near, require, AccountId, Gas, NearToken, PanicOnDefault, Promise,
    PromiseOrValue, PromiseResult,
};

/// Exact panic string. The demo and the TS sandbox tests grep for it.
pub const PANIC_LISTED: &str = "Vessel is listed: withdrawals locked";

const GAS_FT_TRANSFER: Gas = Gas::from_tgas(10);
const GAS_CALLBACK: Gas = Gas::from_tgas(15);
const GAS_VERIFIER_WITHDRAW: Gas = Gas::from_tgas(30);
/// Storage deposit attached to verifier withdrawals for account registration
/// on the receiving side.
const VERIFIER_STORAGE_DEPOSIT: NearToken = NearToken::from_millinear(100);
const ONE_YOCTO: NearToken = NearToken::from_yoctonear(1);

#[ext_contract(ext_ft)]
trait FungibleToken {
    fn ft_transfer(
        &self,
        receiver_id: AccountId,
        amount: U128,
        memo: Option<String>,
    ) -> PromiseOrValue<()>;
}

#[ext_contract(ext_verifier)]
trait IntentsVerifier {
    fn mt_withdraw(
        &self,
        token_id: String,
        amount: U128,
        receiver_id: AccountId,
    ) -> PromiseOrValue<()>;
}

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct VesselWallet {
    config: WalletConfig,
    /// Direct NEP-141 holdings keyed by token contract account.
    balances: UnorderedMap<AccountId, u128>,
}

#[near(serializers=[borsh, json])]
#[derive(Clone)]
pub struct WalletConfig {
    /// The factory that deployed this wallet. Root of trust for owner changes
    /// and the listing freeze.
    pub nft_contract: AccountId,
    pub token_id: String,
    /// Mirror of the current NFT owner. Updated only by the factory.
    pub owner: AccountId,
    /// NEAR Intents verifier. Optional until the factory pins it.
    pub verifier: Option<AccountId>,
    pub is_listed: bool,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct Holding {
    pub asset_id: String,
    pub balance: U128,
    /// "local" for direct NEP-141 balances, "intents" for verifier-held assets.
    pub source: String,
}

/// Tells the frontend exactly how to read an intents balance. View calls
/// cannot fan out cross-contract on NEAR, so the frontend calls the verifier
/// directly:
/// `view intents.near mt_balance_of({"account_id": "<sub-account>", "token_id": "<token_id>"})`
/// where token_id looks like "nep141:wrap.near".
#[derive(Serialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct IntentsBalanceQuery {
    pub verifier: AccountId,
    pub token_id: String,
    pub method: String,
    pub args_json: String,
}

#[near]
impl VesselWallet {
    /// Called by the factory right after deploy_contract. Only the factory
    /// can be the predecessor here because it is the account that created
    /// and funded this sub-account.
    ///
    /// owner_id is passed explicitly: a mutating context cannot resolve the
    /// NFT owner through a synchronous cross-contract view call, and the
    /// factory already knows it at mint time.
    #[init]
    pub fn new(nft_contract: AccountId, token_id: String, owner_id: AccountId) -> Self {
        require!(
            env::predecessor_account_id() == nft_contract,
            "init can only be called by the NFT factory that deployed this wallet"
        );
        require!(
            env::current_account_id() != owner_id,
            "Owner cannot be the wallet itself"
        );
        Self {
            config: WalletConfig {
                nft_contract,
                token_id,
                owner: owner_id,
                verifier: None,
                is_listed: false,
            },
            balances: UnorderedMap::new(b"b".to_vec()),
        }
    }

    /// Withdraw to a direct NEP-141 contract or through the verifier.
    ///
    /// asset_id forms:
    /// - "nep141:wrap.near" sends a plain ft_transfer of the local balance.
    /// - "intents:nep141:wrap.near" asks the verifier to route the asset to
    ///   its destination. Mainnet deposits live in intents.near; the direct
    ///   ft path covers locally held sandbox assets.
    #[payable]
    pub fn withdraw(
        &mut self,
        asset_id: String,
        amount: U128,
        receiver_id: AccountId,
        memo: Option<String>,
    ) -> Promise {
        self.assert_owner();
        if self.config.is_listed {
            env::panic_str(PANIC_LISTED);
        }
        require!(amount.0 > 0, "Amount must be greater than zero");
        require!(
            receiver_id != env::current_account_id(),
            "Receiver cannot be the wallet itself"
        );

        if let Some(token_contract) = asset_id.strip_prefix("nep141:") {
            let token_contract: AccountId =
                token_contract.parse().expect("Invalid nep141 contract id");
            let held = self.balances.get(&token_contract).unwrap_or(0);
            require!(held >= amount.0, "Insufficient local balance");
            ext_ft::ext(token_contract.clone())
                .with_attached_deposit(ONE_YOCTO)
                .with_static_gas(GAS_FT_TRANSFER)
                .ft_transfer(receiver_id, amount, memo)
                .then(
                    Self::ext(env::current_account_id())
                        .with_static_gas(GAS_CALLBACK)
                        .on_ft_withdrawn(token_contract, amount, self.config.owner.clone()),
                )
        } else if let Some(verifier_token) = asset_id.strip_prefix("intents:") {
            let verifier_token = verifier_token.to_string();
            let verifier = self
                .config
                .verifier
                .clone()
                .expect("Verifier account not set for this wallet");
            ext_verifier::ext(verifier)
                .with_attached_deposit(VERIFIER_STORAGE_DEPOSIT)
                .with_static_gas(GAS_VERIFIER_WITHDRAW)
                .mt_withdraw(verifier_token, amount, receiver_id)
        } else {
            env::panic_str("Unsupported asset id, expected nep141: or intents: prefix");
        }
    }

    /// NEP-141 deposit receiver. Any token contract can deposit; the balance
    /// is tracked under the token contract account. Intents holdings are not
    /// recorded here, they are read live from the verifier.
    pub fn ft_on_transfer(&mut self, sender_id: AccountId, amount: U128, msg: String) -> U128 {
        require!(
            sender_id != env::current_account_id(),
            "Self-deposits are not supported"
        );
        let token_contract = env::predecessor_account_id();
        let current = self.balances.get(&token_contract).unwrap_or(0);
        self.balances.insert(&token_contract, &(current + amount.0));
        near_sdk::log!(
            "wallet: deposit {} from {} via {} (msg: {})",
            amount.0,
            sender_id,
            token_contract,
            msg
        );
        U128(0)
    }

    /// Factory only. The factory only triggers this from lock_subaccount,
    /// which the marketplace reaches through the factory, so the marketplace
    /// never needs direct access here.
    pub fn set_listed(&mut self, listed: bool) {
        self.assert_factory();
        self.config.is_listed = listed;
        near_sdk::log!("wallet: is_listed set to {}", listed);
    }

    /// Factory only. Sent after an NFT transfer resolves so the recorded
    /// owner always tracks the NFT owner.
    pub fn owner_changed(&mut self, new_owner: AccountId) {
        self.assert_factory();
        require!(
            new_owner != env::current_account_id(),
            "Owner cannot be the wallet itself"
        );
        self.config.owner = new_owner;
    }

    /// Factory only, one time.
    pub fn set_verifier(&mut self, verifier: AccountId) {
        self.assert_factory();
        require!(
            self.config.verifier.is_none(),
            "Verifier already set for this wallet"
        );
        self.config.verifier = Some(verifier);
    }

    // ----- views -----

    /// Local NEP-141 holdings only. For intents holdings the frontend calls
    /// the verifier directly, see get_intents_balance.
    pub fn get_local_holdings(&self) -> Vec<Holding> {
        self.balances
            .iter()
            .map(|(token, balance)| Holding {
                asset_id: format!("nep141:{}", token),
                balance: U128(balance),
                source: "local".to_string(),
            })
            .collect()
    }

    /// Returns the exact verifier query the frontend should run for an
    /// intents asset. NEAR view calls cannot fan out cross-contract, so this
    /// contract cannot read intents.near synchronously; the query is
    /// described instead of executed.
    pub fn get_intents_balance(&self, asset_id: String) -> Option<IntentsBalanceQuery> {
        let verifier = self.config.verifier.as_ref()?;
        let token_id = asset_id.strip_prefix("intents:")?.to_string();
        Some(IntentsBalanceQuery {
            verifier: verifier.clone(),
            method: "mt_balance_of".to_string(),
            args_json: near_sdk::serde_json::json!({
                "account_id": env::current_account_id(),
                "token_id": token_id,
            })
            .to_string(),
            token_id,
        })
    }

    pub fn get_is_listed(&self) -> bool {
        self.config.is_listed
    }

    pub fn get_owner(&self) -> AccountId {
        self.config.owner.clone()
    }

    pub fn get_config(&self) -> WalletConfig {
        self.config.clone()
    }

    // ----- internal -----

    fn assert_factory(&self) {
        require!(
            env::predecessor_account_id() == self.config.nft_contract,
            "Only the Vessel factory can call this method"
        );
    }

    fn assert_owner(&self) {
        require!(
            env::predecessor_account_id() == self.config.owner,
            "Only the current Vessel NFT owner can withdraw"
        );
    }

    /// Promise callback for the direct ft path. Only on success is the local
    /// balance reduced; on failure the 1 yoctoNEAR that traveled with the
    /// ft_transfer is refunded to the owner.
    #[private]
    pub fn on_ft_withdrawn(
        &mut self,
        token_contract: AccountId,
        amount: U128,
        refund_to: AccountId,
    ) -> bool {
        if matches!(env::promise_result(0), PromiseResult::Successful(_)) {
            let current = self.balances.get(&token_contract).unwrap_or(0);
            self.balances.insert(&token_contract, &(current - amount.0));
            true
        } else {
            Promise::new(refund_to).transfer(ONE_YOCTO).detach();
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::test_utils::{accounts, VMContextBuilder};
    use near_sdk::{testing_env, NearToken};

    const FACTORY: usize = 3;
    const OWNER: usize = 0;
    const WRAP: &str = "wrap.near";

    fn wallet() -> VesselWallet {
        testing_env!(VMContextBuilder::new()
            .current_account_id(accounts(2))
            .predecessor_account_id(accounts(FACTORY))
            .build());
        VesselWallet::new(accounts(FACTORY), "000001".to_string(), accounts(OWNER))
    }

    fn ctx(wallet_id: usize, predecessor: usize) -> VMContextBuilder {
        let mut builder = VMContextBuilder::new();
        builder
            .current_account_id(accounts(wallet_id))
            .predecessor_account_id(accounts(predecessor))
            .signer_account_id(accounts(predecessor));
        builder
    }

    #[test]
    #[should_panic(
        expected = "init can only be called by the NFT factory that deployed this wallet"
    )]
    fn init_rejects_non_factory_caller() {
        testing_env!(VMContextBuilder::new()
            .current_account_id(accounts(2))
            .predecessor_account_id(accounts(0))
            .build());
        VesselWallet::new(accounts(FACTORY), "000001".to_string(), accounts(OWNER));
    }

    #[test]
    fn init_records_config() {
        let w = wallet();
        let cfg = w.get_config();
        assert_eq!(cfg.nft_contract, accounts(FACTORY));
        assert_eq!(cfg.token_id, "000001");
        assert_eq!(cfg.owner, accounts(OWNER));
        assert!(!cfg.is_listed);
        assert!(cfg.verifier.is_none());
        assert_eq!(w.get_owner(), accounts(OWNER));
        assert!(!w.get_is_listed());
    }

    #[test]
    fn withdraw_blocked_when_listed_with_exact_panic() {
        let mut w = wallet();
        testing_env!(ctx(2, FACTORY).build());
        w.set_listed(true);
        testing_env!(ctx(2, OWNER).build());
        let err = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            w.withdraw(format!("nep141:{}", WRAP), U128(1), accounts(1), None);
        }))
        .err()
        .expect("expected panic");
        assert!(err.downcast::<String>().unwrap().contains(PANIC_LISTED));
    }

    #[test]
    fn withdraw_allowed_when_unlisted() {
        let mut w = wallet();
        testing_env!(ctx(2, FACTORY).build());
        w.set_verifier(accounts(4));
        w.set_listed(false);
        testing_env!(ctx(2, OWNER).build());
        let promise = w.withdraw(
            "intents:nep141:wrap.near".to_string(),
            U128(1),
            accounts(1),
            None,
        );
        let _promise = promise; // one function_call receipt scheduled
    }

    #[test]
    #[should_panic(expected = "Only the current Vessel NFT owner can withdraw")]
    fn withdraw_rejects_non_owner() {
        let mut w = wallet();
        testing_env!(ctx(2, 1).build());
        w.withdraw(format!("nep141:{}", WRAP), U128(1), accounts(1), None);
    }

    #[test]
    fn ft_on_transfer_credits_local_balance() {
        let mut w = wallet();
        // predecessor is the token contract, sender is the depositor.
        testing_env!(ctx(2, 5).build());
        let leftover = w.ft_on_transfer(accounts(0), U128(1_500), "seed".to_string());
        assert_eq!(leftover.0, 0);
        let holdings = w.get_local_holdings();
        assert_eq!(holdings.len(), 1);
        assert_eq!(holdings[0].asset_id, format!("nep141:{}", accounts(5)));
        assert_eq!(holdings[0].balance.0, 1_500);
        assert_eq!(holdings[0].source, "local");
    }

    #[test]
    #[should_panic(expected = "Only the Vessel factory can call this method")]
    fn set_listed_rejects_non_factory() {
        let mut w = wallet();
        testing_env!(ctx(2, OWNER).build());
        w.set_listed(true);
    }

    #[test]
    #[should_panic(expected = "Only the Vessel factory can call this method")]
    fn owner_changed_rejects_non_factory() {
        let mut w = wallet();
        testing_env!(ctx(2, OWNER).build());
        w.owner_changed(accounts(1));
    }

    #[test]
    fn set_verifier_is_factory_only_and_one_time() {
        let mut w = wallet();
        testing_env!(ctx(2, FACTORY).build());
        w.set_verifier(accounts(4));
        assert_eq!(w.get_config().verifier, Some(accounts(4)));

        // Second set is rejected even by the factory.
        let err = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            w.set_verifier(accounts(5));
        }))
        .err()
        .expect("expected panic");
        assert_eq!(
            *err.downcast::<String>().unwrap(),
            "Verifier already set for this wallet"
        );
    }

    #[test]
    fn intents_balance_query_describes_verifier_call() {
        let mut w = wallet();
        testing_env!(ctx(2, FACTORY).build());
        w.set_verifier("intents.near".parse().unwrap());

        testing_env!(ctx(2, 0).build());
        let q = w
            .get_intents_balance("intents:nep141:wrap.near".to_string())
            .expect("query expected");
        assert_eq!(q.verifier, "intents.near".to_string());
        assert_eq!(q.method, "mt_balance_of");
        assert!(q.args_json.contains("mt_balance_of") == false);
        assert!(q.args_json.contains(accounts(2).as_str()));

        // Unsupported prefix yields nothing.
        assert!(w
            .get_intents_balance("nep141:wrap.near".to_string())
            .is_none());
    }

    #[test]
    fn withdraw_unsupported_prefix_panics() {
        let mut w = wallet();
        testing_env!(ctx(2, OWNER).build());
        let err = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            w.withdraw("sol:abc".to_string(), U128(1), accounts(1), None);
        }))
        .err()
        .expect("expected panic");
        assert!(err
            .downcast::<String>()
            .unwrap()
            .contains("Unsupported asset id, expected nep141: or intents: prefix"));
    }
}
