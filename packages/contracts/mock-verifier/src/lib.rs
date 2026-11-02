//! Mock stand-in for the NEAR Intents verifier (intents.near).
//!
//! Sandbox and unit tests only. It mimics the small slice of the real
//! verifier ABI that the Vessel contracts use: `mt_balance_of` for balance
//! reads and `mt_withdraw` for cross-chain withdrawals.
//!
//! Never deploy this to mainnet. The real verifier has a different
//! `mt_withdraw` argument shape and stronger authorization; the adapter for
//! the real verifier lives in the TypeScript SDK, not in the wallet contract.

use near_sdk::collections::LookupMap;
use near_sdk::json_types::U128;
use near_sdk::{env, near, require, AccountId, PanicOnDefault};

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct MockVerifier {
    balances: LookupMap<(AccountId, String), u128>,
}

#[near]
impl MockVerifier {
    #[init]
    pub fn new() -> Self {
        Self {
            balances: LookupMap::new(b"v".to_vec()),
        }
    }

    /// Test helper. The real verifier accepts deposits through ft_on_transfer
    /// and off-chain intent settlement; the mock credits the caller with the
    /// attached NEAR for the given synthetic token id.
    #[payable]
    pub fn deposit(&mut self, token_id: String) {
        let amount = env::attached_deposit().as_yoctonear();
        require!(amount > 0, "Deposit must be greater than zero");
        let key = (env::predecessor_account_id(), token_id.clone());
        let current = self.balances.get(&key).unwrap_or(0);
        self.balances.insert(&key, &(current + amount));
        near_sdk::log!("mock-verifier: deposit {} of {}", amount, token_id);
    }

    /// Mirrors the real verifier signature: mt_balance_of(account_id, token_id) -> U128.
    pub fn mt_balance_of(&self, account_id: AccountId, token_id: String) -> U128 {
        self.balances
            .get(&(account_id, token_id))
            .unwrap_or(0)
            .into()
    }

    /// Mirrors the sandbox ABI used by the Vessel wallet contract. The real
    /// intents.near mt_withdraw takes a JSON object with receiver_id, token
    /// and amount fields; the SDK adapts between the two shapes.
    #[payable]
    pub fn mt_withdraw(&mut self, token_id: String, amount: U128, receiver_id: AccountId) {
        let caller = env::predecessor_account_id();
        let key = (caller.clone(), token_id.clone());
        let current = self.balances.get(&key).unwrap_or(0);
        require!(current >= amount.0, "Insufficient balance for withdrawal");
        self.balances.insert(&key, &(current - amount.0));
        near_sdk::log!(
            "mock-verifier: withdraw {} of {} to {}",
            amount.0,
            token_id,
            receiver_id
        );
        // The withdrawal is recorded, not routed. A real verifier would bridge
        // the asset to the destination chain here.
    }

    pub fn get_version(&self) -> String {
        "mock-verifier-1.0.0".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::test_utils::{accounts, VMContextBuilder};
    use near_sdk::{testing_env, NearToken};

    fn ctx() -> VMContextBuilder {
        let mut builder = VMContextBuilder::new();
        builder
            .current_account_id(accounts(3))
            .predecessor_account_id(accounts(0))
            .signer_account_id(accounts(0));
        builder
    }

    fn verifier() -> MockVerifier {
        testing_env!(ctx().build());
        MockVerifier::new()
    }

    #[test]
    fn version_is_stable() {
        let v = verifier();
        assert_eq!(v.get_version(), "mock-verifier-1.0.0");
    }

    #[test]
    fn deposit_credits_caller_balance() {
        let mut v = verifier();
        testing_env!(ctx().attached_deposit(NearToken::from_near(5)).build());
        v.deposit("nep141:wrap.near".to_string());

        testing_env!(ctx().build());
        assert_eq!(
            v.mt_balance_of(accounts(0), "nep141:wrap.near".to_string())
                .0,
            5_000_000_000_000_000_000_000_000
        );
        assert_eq!(
            v.mt_balance_of(accounts(1), "nep141:wrap.near".to_string())
                .0,
            0
        );
    }

    #[test]
    #[should_panic(expected = "Deposit must be greater than zero")]
    fn zero_deposit_panics() {
        let mut v = verifier();
        testing_env!(ctx().build());
        v.deposit("nep141:wrap.near".to_string());
    }

    #[test]
    fn withdraw_decrements_and_over_withdraw_panics() {
        let mut v = verifier();
        testing_env!(ctx().attached_deposit(NearToken::from_near(2)).build());
        v.deposit("nep141:usdt.tkn-primitives.near".to_string());

        testing_env!(ctx().build());
        v.mt_withdraw(
            "nep141:usdt.tkn-primitives.near".to_string(),
            U128(1_000_000),
            accounts(1),
        );
        assert_eq!(
            v.mt_balance_of(accounts(0), "nep141:usdt.tkn-primitives.near".to_string())
                .0,
            2_000_000_000_000_000_000_000_000 - 1_000_000
        );
    }

    #[test]
    #[should_panic(expected = "Insufficient balance for withdrawal")]
    fn over_withdraw_panics() {
        let mut v = verifier();
        testing_env!(ctx().attached_deposit(NearToken::from_near(1)).build());
        v.deposit("nep141:usdt.tkn-primitives.near".to_string());

        testing_env!(ctx().build());
        v.mt_withdraw(
            "nep141:usdt.tkn-primitives.near".to_string(),
            U128(u128::MAX),
            accounts(1),
        );
    }
}
