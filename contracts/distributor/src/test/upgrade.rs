use super::{build_test_tree, setup_test, SimpleClaimItem};
use crate::errors::ContractError;
use crate::types::DistributionStatus;
use crate::{YieldStreamDistributor, YieldStreamDistributorClient};
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events},
    Address, Env, Symbol, TryFromVal,
};

const V2_WASM: &[u8] = include_bytes!("../../../../test-vectors/distributor_v2.wasm");

#[test]
fn test_upgrade_contract_success_and_preserves_state() {
    let env = Env::default();
    let fixture = setup_test(&env);

    // 1. Initialize contract and verify version 1
    fixture.client.initialize(&fixture.admin);
    assert_eq!(fixture.client.get_version(), 1);
    assert_eq!(fixture.client.get_admin(), fixture.admin);

    // 2. Setup a 2-recipient distribution under v1
    let rec0 = Address::generate(&env);
    let rec1 = Address::generate(&env);
    let amount0 = 100_000i128;
    let amount1 = 200_000i128;
    let total_amount = amount0 + amount1;

    let items = [
        SimpleClaimItem {
            index: 0,
            recipient: rec0.clone(),
            amount: amount0,
        },
        SimpleClaimItem {
            index: 1,
            recipient: rec1.clone(),
            amount: amount1,
        },
    ];

    let tree = build_test_tree(&env, 1, &items);

    let dist_id = fixture.client.create_distribution(
        &fixture.creator,
        &fixture.token_id,
        &tree.root,
        &total_amount,
        &2,
        &0,
    );
    assert_eq!(dist_id, 1);
    assert_eq!(fixture.client.get_distribution_count(), 1);

    // 3. Claim leaf 0 on v1
    fixture
        .client
        .claim(&dist_id, &0, &rec0, &amount0, &tree.proofs[0]);
    assert_eq!(fixture.token.balance(&rec0), amount0);
    assert!(fixture.client.is_claimed(&dist_id, &0));
    assert!(!fixture.client.is_claimed(&dist_id, &1));

    let dist_v1 = fixture.client.get_distribution(&dist_id);
    assert_eq!(dist_v1.claimed_amount, amount0);
    assert_eq!(dist_v1.claimed_count, 1);
    assert_eq!(dist_v1.status, DistributionStatus::Active);

    // 4. Upload v2 WASM and upgrade contract
    let v2_wasm_hash = env.deployer().upload_contract_wasm(V2_WASM);
    fixture.client.upgrade(&v2_wasm_hash);

    // 5. Verify upgrade event was emitted
    let events = env.events().all();
    let upgrade_symbol = symbol_short!("upgrade");
    let upgrade_event_found = events.iter().any(|ev| {
        ev.1.iter()
            .any(|topic| Symbol::try_from_val(&env, &topic) == Ok(upgrade_symbol.clone()))
    });
    assert!(
        upgrade_event_found,
        "Contract upgrade event must be emitted"
    );

    // 6. Verify version upgraded to 2
    assert_eq!(fixture.client.get_version(), 2);

    // 7. Verify all v1 storage keys are intact post-upgrade
    assert_eq!(fixture.client.get_admin(), fixture.admin);
    assert_eq!(fixture.client.get_distribution_count(), 1);
    assert!(fixture.client.is_claimed(&dist_id, &0));
    assert!(!fixture.client.is_claimed(&dist_id, &1));

    let dist_post = fixture.client.get_distribution(&dist_id);
    assert_eq!(dist_post.id, dist_v1.id);
    assert_eq!(dist_post.creator, dist_v1.creator);
    assert_eq!(dist_post.token, dist_v1.token);
    assert_eq!(dist_post.merkle_root, dist_v1.merkle_root);
    assert_eq!(dist_post.total_amount, dist_v1.total_amount);
    assert_eq!(dist_post.claimed_amount, dist_v1.claimed_amount);
    assert_eq!(dist_post.num_recipients, dist_v1.num_recipients);
    assert_eq!(dist_post.claimed_count, dist_v1.claimed_count);
    assert_eq!(dist_post.status, DistributionStatus::Active);

    // 8. Claim leaf 1 on v2 with state intact
    fixture
        .client
        .claim(&dist_id, &1, &rec1, &amount1, &tree.proofs[1]);
    assert_eq!(fixture.token.balance(&rec1), amount1);
    assert!(fixture.client.is_claimed(&dist_id, &1));

    let dist_final = fixture.client.get_distribution(&dist_id);
    assert_eq!(dist_final.claimed_amount, total_amount);
    assert_eq!(dist_final.claimed_count, 2);
    assert_eq!(dist_final.status, DistributionStatus::Completed);

    // 9. Verify double-claim rejection persists across upgrade
    let res_double_claim_0 =
        fixture
            .client
            .try_claim(&dist_id, &0, &rec0, &amount0, &tree.proofs[0]);
    assert_eq!(res_double_claim_0, Err(Ok(ContractError::AlreadyClaimed)));

    let res_double_claim_1 =
        fixture
            .client
            .try_claim(&dist_id, &1, &rec1, &amount1, &tree.proofs[1]);
    assert_eq!(res_double_claim_1, Err(Ok(ContractError::AlreadyClaimed)));
}

#[test]
fn test_upgrade_uninitialized_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(YieldStreamDistributor, ());
    let client = YieldStreamDistributorClient::new(&env, &contract_id);

    let v2_wasm_hash = env.deployer().upload_contract_wasm(V2_WASM);

    // Attempting upgrade on an uninitialized contract must fail with NotInitialized
    let res = client.try_upgrade(&v2_wasm_hash);
    assert_eq!(res, Err(Ok(ContractError::NotInitialized)));
}

#[test]
fn test_upgrade_consecutive_versions() {
    let env = Env::default();
    let fixture = setup_test(&env);

    fixture.client.initialize(&fixture.admin);
    assert_eq!(fixture.client.get_version(), 1);

    let v2_wasm_hash = env.deployer().upload_contract_wasm(V2_WASM);

    // First upgrade -> version 2
    fixture.client.upgrade(&v2_wasm_hash);
    assert_eq!(fixture.client.get_version(), 2);

    // Second upgrade -> version 3
    fixture.client.upgrade(&v2_wasm_hash);
    assert_eq!(fixture.client.get_version(), 3);
}

#[test]
fn test_upgrade_auth_matrix() {
    let env = Env::default();
    let fixture = setup_test(&env);

    fixture.client.initialize(&fixture.admin);

    let v2_wasm_hash = env.deployer().upload_contract_wasm(V2_WASM);

    // Clear recorded auths from initialize
    fixture.client.upgrade(&v2_wasm_hash);

    // Verify that the authorization required is specifically from the admin address
    let auths = env.auths();
    assert_eq!(auths.len(), 1, "Must require exactly one authorization");
    assert_eq!(
        auths[0].0, fixture.admin,
        "Required auth must be the admin address"
    );
}

#[test]
#[should_panic]
fn test_upgrade_without_admin_auth_fails() {
    let env = Env::default();
    let contract_id = env.register(YieldStreamDistributor, ());
    let client = YieldStreamDistributorClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    env.mock_all_auths();
    client.initialize(&admin);

    let v2_wasm_hash = env.deployer().upload_contract_wasm(V2_WASM);

    // Disable mocked authorizations so require_auth fails
    env.mock_auths(&[]);
    client.upgrade(&v2_wasm_hash);
}
