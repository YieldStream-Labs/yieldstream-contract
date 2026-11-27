#![no_std]
#![allow(clippy::too_many_arguments)]

pub mod errors;
pub mod events;
pub mod merkle;
pub mod storage;
pub mod types;
pub mod vesting;

#[cfg(test)]
mod test;

use errors::ContractError;
use soroban_sdk::{contract, contractimpl, token, Address, BytesN, Env, Vec};
use types::{Distribution, DistributionStatus, VestingSchedule};

#[contract]
pub struct YieldStreamDistributor;

#[contractimpl]
impl YieldStreamDistributor {
    /// Initializes the contract with an administrative address.
    pub fn initialize(env: Env, admin: Address) -> Result<(), ContractError> {
        if storage::has_admin(&env) {
            return Err(ContractError::AlreadyInitialized);
        }
        admin.require_auth();
        storage::set_admin(&env, &admin);
        storage::set_version(&env, 1);
        Ok(())
    }

    /// Upgrades the contract executable Wasm code to a new uploaded Wasm hash.
    /// Restricted to contract administrator.
    pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>) -> Result<(), ContractError> {
        let admin = storage::get_admin(&env)?;
        admin.require_auth();
        storage::bump_instance(&env);

        let current_version = storage::get_version(&env);
        let new_version = current_version
            .checked_add(1)
            .ok_or(ContractError::ArithmeticOverflow)?;
        storage::set_version(&env, new_version);

        env.deployer()
            .update_current_contract_wasm(new_wasm_hash.clone());
        events::contract_upgraded(&env, current_version, new_version, &new_wasm_hash);

        Ok(())
    }

    /// Creates a new distribution campaign, transferring total tokens to the contract.
    pub fn create_distribution(
        env: Env,
        creator: Address,
        token: Address,
        merkle_root: BytesN<32>,
        total_amount: i128,
        num_recipients: u32,
        end_time: u64,
    ) -> Result<u32, ContractError> {
        creator.require_auth();

        if total_amount <= 0 {
            return Err(ContractError::InvalidAmount);
        }
        if num_recipients == 0 {
            return Err(ContractError::InvalidRecipientCount);
        }
        if end_time != 0 && end_time <= env.ledger().timestamp() {
            return Err(ContractError::InvalidEndTime);
        }

        let next_id = create_and_fund(
            &env,
            &creator,
            &token,
            &merkle_root,
            total_amount,
            num_recipients,
            end_time,
        )?;

        events::distribution_created(
            &env,
            next_id,
            &creator,
            &token,
            &merkle_root,
            total_amount,
            end_time,
        );

        Ok(next_id)
    }

    /// Creates a vesting stream with a cliff and linear unlock schedule.
    ///
    /// The issuer funds the stream immediately. Recipients can claim only the
    /// portion vested at the current ledger timestamp, subject to the cliff.
    pub fn create_stream(
        env: Env,
        issuer: Address,
        token: Address,
        merkle_root: BytesN<32>,
        total_amount: i128,
        num_recipients: u32,
        start_time: u64,
        cliff_time: u64,
        end_time: u64,
    ) -> Result<u32, ContractError> {
        issuer.require_auth();

        if total_amount <= 0 {
            return Err(ContractError::InvalidAmount);
        }
        if num_recipients == 0 {
            return Err(ContractError::InvalidRecipientCount);
        }
        if end_time <= start_time || cliff_time < start_time || cliff_time > end_time {
            return Err(ContractError::InvalidVestingSchedule);
        }

        let next_id = create_and_fund(
            &env,
            &issuer,
            &token,
            &merkle_root,
            total_amount,
            num_recipients,
            0,
        )?;

        let schedule = VestingSchedule {
            start_time,
            cliff_time,
            end_time,
        };
        storage::set_vesting_schedule(&env, next_id, &schedule);

        events::stream_created(
            &env,
            next_id,
            &issuer,
            &token,
            &merkle_root,
            total_amount,
            start_time,
            cliff_time,
            end_time,
        );

        Ok(next_id)
    }

    /// Claims an allocated share of a distribution using a Merkle inclusion proof.
    pub fn claim(
        env: Env,
        distribution_id: u32,
        index: u32,
        recipient: Address,
        amount: i128,
        proof: Vec<BytesN<32>>,
    ) -> Result<(), ContractError> {
        recipient.require_auth();
        storage::bump_instance(&env);

        let mut distribution = storage::get_distribution(&env, distribution_id)?;

        if amount <= 0 {
            return Err(ContractError::InvalidAmount);
        }
        if index >= distribution.num_recipients {
            return Err(ContractError::InvalidLeafIndex);
        }
        if storage::is_claimed(&env, distribution_id, index) {
            return Err(ContractError::AlreadyClaimed);
        }
        if distribution.status == DistributionStatus::Cancelled {
            return Err(ContractError::DistributionCancelled);
        }
        if distribution.status != DistributionStatus::Active
            && distribution.status != DistributionStatus::ClawedBack
        {
            return Err(ContractError::DistributionNotActive);
        }

        let now = env.ledger().timestamp();
        if distribution.end_time > 0 && now >= distribution.end_time {
            return Err(ContractError::DistributionExpired);
        }

        let already_claimed = storage::get_claimed_amount(&env, distribution_id, index);
        if already_claimed >= amount {
            return Err(ContractError::AlreadyClaimed);
        }

        let leaf = merkle::compute_leaf(&env, distribution_id, index, &recipient, amount);
        if !merkle::verify_merkle_proof(&env, &proof, &distribution.merkle_root, &leaf) {
            return Err(ContractError::InvalidProof);
        }

        let clawback_time = storage::get_clawback_time(&env, distribution_id);
        let vested = match storage::get_vesting_schedule(&env, distribution_id) {
            Some(schedule) => {
                let vesting_time = clawback_time.unwrap_or(now);
                vesting::vested_amount(&schedule, amount, vesting_time)?
            }
            None => amount,
        };
        let claimable = vested
            .checked_sub(already_claimed)
            .ok_or(ContractError::ArithmeticOverflow)?;
        if claimable <= 0 {
            return Err(ContractError::NothingToClaim);
        }

        let leaf_claimed = already_claimed
            .checked_add(claimable)
            .ok_or(ContractError::ArithmeticOverflow)?;
        storage::set_claimed_amount(&env, distribution_id, index, leaf_claimed);

        let fully_claimed =
            leaf_claimed >= amount || (clawback_time.is_some() && leaf_claimed >= vested);
        if fully_claimed {
            storage::set_claimed(&env, distribution_id, index);
            distribution.claimed_count = distribution
                .claimed_count
                .checked_add(1)
                .ok_or(ContractError::ArithmeticOverflow)?;
        }

        distribution.claimed_amount = distribution
            .claimed_amount
            .checked_add(claimable)
            .ok_or(ContractError::ArithmeticOverflow)?;

        if distribution.claimed_count == distribution.num_recipients
            || distribution.claimed_amount == distribution.total_amount
        {
            distribution.status = DistributionStatus::Completed;
        }

        storage::set_distribution(&env, distribution_id, &distribution);

        let contract_address = env.current_contract_address();
        token::Client::new(&env, &distribution.token).transfer(
            &contract_address,
            &recipient,
            &claimable,
        );

        events::distribution_claimed(&env, distribution_id, index, &recipient, claimable);

        Ok(())
    }

    /// Cancels an active distribution, halting future claims.
    pub fn cancel(env: Env, distribution_id: u32) -> Result<(), ContractError> {
        storage::bump_instance(&env);
        let mut distribution = storage::get_distribution(&env, distribution_id)?;
        distribution.creator.require_auth();

        if distribution.status != DistributionStatus::Active {
            return Err(ContractError::DistributionNotActive);
        }

        distribution.status = DistributionStatus::Cancelled;
        let remaining_amount = distribution
            .total_amount
            .checked_sub(distribution.claimed_amount)
            .ok_or(ContractError::ArithmeticOverflow)?;

        storage::set_distribution(&env, distribution_id, &distribution);
        events::distribution_cancelled(
            &env,
            distribution_id,
            &distribution.creator,
            remaining_amount,
        );

        Ok(())
    }

    /// Claws back unvested tokens from a stream to the issuer.
    ///
    /// The vested-but-unclaimed balance stays in the contract for recipients.
    /// Vesting is frozen at the clawback timestamp so no additional tokens vest.
    pub fn clawback(env: Env, distribution_id: u32) -> Result<i128, ContractError> {
        storage::bump_instance(&env);
        let mut distribution = storage::get_distribution(&env, distribution_id)?;
        distribution.creator.require_auth();

        if storage::get_clawback_time(&env, distribution_id).is_some() {
            return Err(ContractError::AlreadyClawedBack);
        }
        if distribution.status != DistributionStatus::Active {
            return Err(ContractError::DistributionNotActive);
        }

        let schedule = match storage::get_vesting_schedule(&env, distribution_id) {
            Some(schedule) => schedule,
            None => return Err(ContractError::NothingToClawback),
        };

        let now = env.ledger().timestamp();
        let vested_total = vesting::vested_amount(&schedule, distribution.total_amount, now)?;
        let unvested = distribution
            .total_amount
            .checked_sub(vested_total)
            .ok_or(ContractError::ArithmeticOverflow)?;
        if unvested <= 0 {
            return Err(ContractError::NothingToClawback);
        }

        storage::set_clawback_time(&env, distribution_id, now);
        distribution.status = DistributionStatus::ClawedBack;
        storage::set_distribution(&env, distribution_id, &distribution);

        let contract_address = env.current_contract_address();
        token::Client::new(&env, &distribution.token).transfer(
            &contract_address,
            &distribution.creator,
            &unvested,
        );

        events::stream_clawed_back(&env, distribution_id, &distribution.creator, unvested, now);

        Ok(unvested)
    }

    /// Sweeps remaining unclaimed tokens to a recipient address if cancelled or expired.
    pub fn sweep(env: Env, distribution_id: u32, to: Address) -> Result<i128, ContractError> {
        storage::bump_instance(&env);
        let mut distribution = storage::get_distribution(&env, distribution_id)?;
        distribution.creator.require_auth();

        if distribution.status == DistributionStatus::Completed {
            return Err(ContractError::NothingToSweep);
        }

        let is_cancelled = distribution.status == DistributionStatus::Cancelled;
        let is_expired =
            distribution.end_time > 0 && env.ledger().timestamp() >= distribution.end_time;

        if !is_cancelled && !is_expired {
            return Err(ContractError::DistributionNotExpired);
        }

        let remaining_amount = distribution
            .total_amount
            .checked_sub(distribution.claimed_amount)
            .ok_or(ContractError::ArithmeticOverflow)?;

        if remaining_amount <= 0 {
            return Err(ContractError::NothingToSweep);
        }

        distribution.status = DistributionStatus::Completed;
        distribution.claimed_amount = distribution.total_amount;
        storage::set_distribution(&env, distribution_id, &distribution);

        let contract_address = env.current_contract_address();
        token::Client::new(&env, &distribution.token).transfer(
            &contract_address,
            &to,
            &remaining_amount,
        );

        events::distribution_swept(
            &env,
            distribution_id,
            &distribution.creator,
            &to,
            remaining_amount,
        );

        Ok(remaining_amount)
    }

    /// View: Retrieves distribution metadata by ID.
    pub fn get_distribution(env: Env, distribution_id: u32) -> Result<Distribution, ContractError> {
        storage::get_distribution(&env, distribution_id)
    }

    /// View: Retrieves the vesting schedule for a stream, if configured.
    pub fn get_vesting_schedule(env: Env, distribution_id: u32) -> Option<VestingSchedule> {
        storage::get_vesting_schedule(&env, distribution_id)
    }

    /// View: Gets the cumulative amount already claimed for a leaf.
    pub fn get_claimed_amount(env: Env, distribution_id: u32, index: u32) -> i128 {
        storage::get_claimed_amount(&env, distribution_id, index)
    }

    /// View: Gets the ledger timestamp when a stream was clawed back, if any.
    pub fn get_clawback_time(env: Env, distribution_id: u32) -> Option<u64> {
        storage::get_clawback_time(&env, distribution_id)
    }

    /// View: Computes the amount currently claimable for a leaf.
    pub fn get_claimable_amount(
        env: Env,
        distribution_id: u32,
        index: u32,
        amount: i128,
    ) -> Result<i128, ContractError> {
        if amount <= 0 {
            return Err(ContractError::InvalidAmount);
        }

        let distribution = storage::get_distribution(&env, distribution_id)?;
        if index >= distribution.num_recipients {
            return Err(ContractError::InvalidLeafIndex);
        }

        let already_claimed = storage::get_claimed_amount(&env, distribution_id, index);
        let now = env.ledger().timestamp();
        let vested = match storage::get_vesting_schedule(&env, distribution_id) {
            Some(schedule) => {
                let vesting_time = storage::get_clawback_time(&env, distribution_id).unwrap_or(now);
                vesting::vested_amount(&schedule, amount, vesting_time)?
            }
            None => amount,
        };

        if vested > already_claimed {
            Ok(vested - already_claimed)
        } else {
            Ok(0)
        }
    }

    /// View: Checks whether a specific leaf index has been claimed.
    pub fn is_claimed(env: Env, distribution_id: u32, index: u32) -> bool {
        storage::is_claimed(&env, distribution_id, index)
    }

    /// View: Gets the total number of distributions created.
    pub fn get_distribution_count(env: Env) -> u32 {
        storage::get_distribution_count(&env)
    }

    /// View: Gets the initialized contract administrator.
    pub fn get_admin(env: Env) -> Result<Address, ContractError> {
        storage::get_admin(&env)
    }

    /// View: Gets the current contract version.
    pub fn get_version(env: Env) -> u32 {
        storage::get_version(&env)
    }

    /// Helper: Computes the leaf hash for given distribution claim parameters.
    pub fn compute_leaf(
        env: Env,
        distribution_id: u32,
        index: u32,
        recipient: Address,
        amount: i128,
    ) -> BytesN<32> {
        merkle::compute_leaf(&env, distribution_id, index, &recipient, amount)
    }

    /// Helper: Verifies a Merkle proof against a root and leaf.
    pub fn verify_proof(
        env: Env,
        proof: Vec<BytesN<32>>,
        root: BytesN<32>,
        leaf: BytesN<32>,
    ) -> bool {
        merkle::verify_merkle_proof(&env, &proof, &root, &leaf)
    }
}

fn create_and_fund(
    env: &Env,
    creator: &Address,
    token: &Address,
    merkle_root: &BytesN<32>,
    total_amount: i128,
    num_recipients: u32,
    end_time: u64,
) -> Result<u32, ContractError> {
    let next_id = storage::get_distribution_count(env)
        .checked_add(1)
        .ok_or(ContractError::ArithmeticOverflow)?;
    storage::set_distribution_count(env, next_id);

    let contract_address = env.current_contract_address();
    token::Client::new(env, token).transfer(creator, &contract_address, &total_amount);

    let distribution = Distribution {
        id: next_id,
        creator: creator.clone(),
        token: token.clone(),
        merkle_root: merkle_root.clone(),
        total_amount,
        claimed_amount: 0,
        num_recipients,
        claimed_count: 0,
        created_at: env.ledger().timestamp(),
        end_time,
        status: DistributionStatus::Active,
    };

    storage::set_distribution(env, next_id, &distribution);
    Ok(next_id)
}
