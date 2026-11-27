use super::{build_test_tree, setup_test, SimpleClaimItem};
use crate::errors::ContractError;
use crate::types::DistributionStatus;
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events, Ledger},
    token, Address, Env, Symbol, TryFromVal,
};

const START_TIME: u64 = 1_000;
const CLIFF_TIME: u64 = 2_000;
const END_TIME: u64 = 11_000;

fn build_single_stream<'a>(
    env: &'a Env,
    fixture: &super::TestFixture<'a>,
    recipient: &Address,
    amount: i128,
) -> (u32, soroban_sdk::Vec<soroban_sdk::BytesN<32>>) {
    let items = [SimpleClaimItem {
        index: 0,
        recipient: recipient.clone(),
        amount,
    }];
    let tree = build_test_tree(env, 1, &items);
    let dist_id = fixture.client.create_stream(
        &fixture.creator,
        &fixture.token_id,
        &tree.root,
        &amount,
        &1,
        &START_TIME,
        &CLIFF_TIME,
        &END_TIME,
    );
    (dist_id, tree.proofs[0].clone())
}

#[test]
fn test_stream_before_cliff_has_zero_claimable() {
    let env = Env::default();
    let fixture = setup_test(&env);

    let recipient = Address::generate(&env);
    let amount = 100_000i128;
    let (dist_id, proof) = build_single_stream(&env, &fixture, &recipient, amount);

    let schedule = fixture
        .client
        .get_vesting_schedule(&dist_id)
        .expect("stream must have a vesting schedule");
    assert_eq!(schedule.start_time, START_TIME);
    assert_eq!(schedule.cliff_time, CLIFF_TIME);
    assert_eq!(schedule.end_time, END_TIME);

    let dist = fixture.client.get_distribution(&dist_id);
    assert_eq!(dist.end_time, 0);
    assert_eq!(dist.status, DistributionStatus::Active);

    // Current timestamp is 1_000, which is before the 2_000 cliff.
    assert_eq!(
        fixture.client.get_claimable_amount(&dist_id, &0, &amount),
        0
    );

    let res = fixture
        .client
        .try_claim(&dist_id, &0, &recipient, &amount, &proof);
    assert_eq!(res, Err(Ok(ContractError::NothingToClaim)));
    assert_eq!(fixture.token.balance(&recipient), 0);
}

#[test]
fn test_stream_mid_vesting_partial_claim() {
    let env = Env::default();
    let fixture = setup_test(&env);

    let recipient = Address::generate(&env);
    let amount = 100_000i128;
    let (dist_id, proof) = build_single_stream(&env, &fixture, &recipient, amount);

    // 5_000 of 10_000 vesting seconds elapsed after the cliff -> 50%.
    env.ledger().set_timestamp(6_000);
    assert_eq!(
        fixture.client.get_claimable_amount(&dist_id, &0, &amount),
        50_000
    );

    fixture
        .client
        .claim(&dist_id, &0, &recipient, &amount, &proof);

    assert_eq!(fixture.token.balance(&recipient), 50_000);
    assert_eq!(fixture.client.get_claimed_amount(&dist_id, &0), 50_000);
    assert!(!fixture.client.is_claimed(&dist_id, &0));

    let dist = fixture.client.get_distribution(&dist_id);
    assert_eq!(dist.claimed_amount, 50_000);
    assert_eq!(dist.claimed_count, 0);
    assert_eq!(dist.status, DistributionStatus::Active);

    // Nothing new has vested, so an immediate second claim is a no-op.
    let res = fixture
        .client
        .try_claim(&dist_id, &0, &recipient, &amount, &proof);
    assert_eq!(res, Err(Ok(ContractError::NothingToClaim)));
}

#[test]
fn test_stream_post_vesting_full_claim() {
    let env = Env::default();
    let fixture = setup_test(&env);

    let recipient = Address::generate(&env);
    let amount = 100_000i128;
    let (dist_id, proof) = build_single_stream(&env, &fixture, &recipient, amount);

    env.ledger().set_timestamp(END_TIME);
    assert_eq!(
        fixture.client.get_claimable_amount(&dist_id, &0, &amount),
        amount
    );

    fixture
        .client
        .claim(&dist_id, &0, &recipient, &amount, &proof);

    assert_eq!(fixture.token.balance(&recipient), amount);
    assert!(fixture.client.is_claimed(&dist_id, &0));

    let dist = fixture.client.get_distribution(&dist_id);
    assert_eq!(dist.claimed_amount, amount);
    assert_eq!(dist.claimed_count, 1);
    assert_eq!(dist.status, DistributionStatus::Completed);
    assert_eq!(fixture.token.balance(&fixture.contract_id), 0);
}

#[test]
fn test_stream_double_claim_after_full_claim_rejected() {
    let env = Env::default();
    let fixture = setup_test(&env);

    let recipient = Address::generate(&env);
    let amount = 100_000i128;
    let (dist_id, proof) = build_single_stream(&env, &fixture, &recipient, amount);

    env.ledger().set_timestamp(END_TIME);
    fixture
        .client
        .claim(&dist_id, &0, &recipient, &amount, &proof);

    let res = fixture
        .client
        .try_claim(&dist_id, &0, &recipient, &amount, &proof);
    assert_eq!(res, Err(Ok(ContractError::AlreadyClaimed)));
}

#[test]
fn test_stream_uses_configured_sac_token() {
    let env = Env::default();
    let fixture = setup_test(&env);

    let other_token_admin = Address::generate(&env);
    let other_token_contract = env.register_stellar_asset_contract_v2(other_token_admin);
    let other_token_id = other_token_contract.address();
    let other_token = token::Client::new(&env, &other_token_id);
    let other_token_admin_client = token::StellarAssetClient::new(&env, &other_token_id);
    other_token_admin_client.mint(&fixture.creator, &1_000_000);

    let recipient = Address::generate(&env);
    let amount = 100_000i128;

    let items = [SimpleClaimItem {
        index: 0,
        recipient: recipient.clone(),
        amount,
    }];
    let tree = build_test_tree(&env, 1, &items);

    let dist_id = fixture.client.create_stream(
        &fixture.creator,
        &other_token_id,
        &tree.root,
        &amount,
        &1,
        &START_TIME,
        &CLIFF_TIME,
        &END_TIME,
    );

    assert_eq!(other_token.balance(&fixture.creator), 900_000);
    assert_eq!(other_token.balance(&fixture.contract_id), amount);
    // The original fixture token must remain untouched.
    assert_eq!(fixture.token.balance(&fixture.creator), 10_000_000);

    env.ledger().set_timestamp(END_TIME);
    fixture
        .client
        .claim(&dist_id, &0, &recipient, &amount, &tree.proofs[0]);

    assert_eq!(other_token.balance(&recipient), amount);
    assert_eq!(other_token.balance(&fixture.contract_id), 0);
}

#[test]
fn test_stream_creation_and_claim_events_emitted() {
    let env = Env::default();
    let fixture = setup_test(&env);

    let recipient = Address::generate(&env);
    let amount = 100_000i128;
    let (dist_id, proof) = build_single_stream(&env, &fixture, &recipient, amount);

    let stream_symbol = symbol_short!("stream");
    let stream_events = env.events().all();
    let stream_event_found = stream_events.iter().any(|event| {
        event
            .1
            .iter()
            .any(|topic| Symbol::try_from_val(&env, &topic) == Ok(stream_symbol.clone()))
    });
    assert!(stream_event_found, "stream creation event must be emitted");

    env.ledger().set_timestamp(END_TIME);
    fixture
        .client
        .claim(&dist_id, &0, &recipient, &amount, &proof);

    let claimed_symbol = symbol_short!("claimed");
    let claim_events = env.events().all();
    let claimed_event_found = claim_events.iter().any(|event| {
        event
            .1
            .iter()
            .any(|topic| Symbol::try_from_val(&env, &topic) == Ok(claimed_symbol.clone()))
    });
    assert!(claimed_event_found, "claim event must be emitted");
}
