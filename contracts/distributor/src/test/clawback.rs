use super::{build_test_tree, setup_test, SimpleClaimItem};
use crate::errors::ContractError;
use crate::types::DistributionStatus;
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events, Ledger},
    Address, Env, Symbol, TryFromVal,
};

const START_TIME: u64 = 1_000;
const CLIFF_TIME: u64 = 2_000;
const END_TIME: u64 = 11_000;

fn create_stream<'a>(
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
fn test_clawback_requires_issuer_auth() {
    let env = Env::default();
    let fixture = setup_test(&env);

    let recipient = Address::generate(&env);
    let amount = 100_000i128;
    let (dist_id, _) = create_stream(&env, &fixture, &recipient, amount);

    env.ledger().set_timestamp(6_000);
    fixture.client.clawback(&dist_id);

    let auths = env.auths();
    assert_eq!(auths.len(), 1, "clawback must require exactly one auth");
    assert_eq!(
        auths[0].0, fixture.creator,
        "clawback auth must be the stream issuer"
    );
}

#[test]
#[should_panic]
fn test_clawback_without_issuer_auth_fails() {
    let env = Env::default();
    let fixture = setup_test(&env);

    let recipient = Address::generate(&env);
    let amount = 100_000i128;
    let (dist_id, _) = create_stream(&env, &fixture, &recipient, amount);

    env.ledger().set_timestamp(6_000);
    env.mock_auths(&[]);
    fixture.client.clawback(&dist_id);
}

#[test]
fn test_clawback_returns_only_unvested_mid_vesting() {
    let env = Env::default();
    let fixture = setup_test(&env);

    let recipient = Address::generate(&env);
    let amount = 100_000i128;
    let (dist_id, proof) = create_stream(&env, &fixture, &recipient, amount);

    // At 6_000 half the stream has vested and half is still unvested.
    env.ledger().set_timestamp(6_000);
    let issuer_before = fixture.token.balance(&fixture.creator);
    let contract_before = fixture.token.balance(&fixture.contract_id);

    let clawed_back = fixture.client.clawback(&dist_id);
    assert_eq!(clawed_back, 50_000);
    assert_eq!(
        fixture.token.balance(&fixture.creator) - issuer_before,
        50_000
    );
    assert_eq!(
        contract_before - fixture.token.balance(&fixture.contract_id),
        50_000
    );

    let dist = fixture.client.get_distribution(&dist_id);
    assert_eq!(dist.status, DistributionStatus::ClawedBack);
    assert_eq!(fixture.client.get_clawback_time(&dist_id), Some(6_000));

    let res = fixture.client.try_clawback(&dist_id);
    assert_eq!(res, Err(Ok(ContractError::AlreadyClawedBack)));

    // Vesting is frozen at clawback time, so only the remaining vested 50_000
    // is still claimable even after the original vesting end has passed.
    env.ledger().set_timestamp(20_000);
    assert_eq!(
        fixture.client.get_claimable_amount(&dist_id, &0, &amount),
        50_000
    );

    fixture
        .client
        .claim(&dist_id, &0, &recipient, &amount, &proof);
    assert_eq!(fixture.client.get_claimed_amount(&dist_id, &0), 50_000);
    assert!(fixture.client.is_claimed(&dist_id, &0));
    assert_eq!(fixture.token.balance(&recipient), 50_000);
    assert_eq!(fixture.token.balance(&fixture.contract_id), 0);

    let res = fixture
        .client
        .try_claim(&dist_id, &0, &recipient, &amount, &proof);
    assert_eq!(res, Err(Ok(ContractError::AlreadyClaimed)));
}

#[test]
fn test_clawback_before_cliff_returns_entire_balance() {
    let env = Env::default();
    let fixture = setup_test(&env);

    let recipient = Address::generate(&env);
    let amount = 100_000i128;
    let (dist_id, _) = create_stream(&env, &fixture, &recipient, amount);

    env.ledger().set_timestamp(1_500);
    let clawed_back = fixture.client.clawback(&dist_id);
    assert_eq!(clawed_back, amount);

    let clawback_symbol = symbol_short!("clawback");
    let events = env.events().all();
    let clawback_event_found = events.iter().any(|event| {
        event
            .1
            .iter()
            .any(|topic| Symbol::try_from_val(&env, &topic) == Ok(clawback_symbol.clone()))
    });
    assert!(clawback_event_found, "clawback event must be emitted");

    assert_eq!(fixture.token.balance(&fixture.contract_id), 0);

    env.ledger().set_timestamp(END_TIME);
    assert_eq!(
        fixture.client.get_claimable_amount(&dist_id, &0, &amount),
        0
    );
}

#[test]
fn test_clawback_after_vesting_complete_returns_nothing() {
    let env = Env::default();
    let fixture = setup_test(&env);

    let recipient = Address::generate(&env);
    let amount = 100_000i128;
    let (dist_id, _) = create_stream(&env, &fixture, &recipient, amount);

    env.ledger().set_timestamp(END_TIME);
    let res = fixture.client.try_clawback(&dist_id);
    assert_eq!(res, Err(Ok(ContractError::NothingToClawback)));
}

#[test]
fn test_clawback_rejects_non_stream_distribution() {
    let env = Env::default();
    let fixture = setup_test(&env);

    let recipient = Address::generate(&env);
    let amount = 100_000i128;
    let items = [SimpleClaimItem {
        index: 0,
        recipient: recipient.clone(),
        amount,
    }];
    let tree = build_test_tree(&env, 1, &items);

    let dist_id = fixture.client.create_distribution(
        &fixture.creator,
        &fixture.token_id,
        &tree.root,
        &amount,
        &1,
        &0,
    );

    let res = fixture.client.try_clawback(&dist_id);
    assert_eq!(res, Err(Ok(ContractError::NothingToClawback)));
}
