# YieldStream Distributor: Contract Upgradability Runbook & Architecture

This document describes the upgrade mechanism for the YieldStream Distributor contract (`YieldStreamDistributor`), operational procedures for executing upgrades, state preservation guarantees, and an architectural analysis of the trust trade-offs and composition with the emergency pause switch ([#9](https://github.com/yieldstream-org/yieldstream-contract/issues/9)).

---

## 1. Overview & Architecture

Soroban contracts use code hash separation:
- **Contract Instance**: Persistent identity (Contract ID / Address), instance storage, and persistent storage entries.
- **Contract Executable**: The WebAssembly (WASM) bytecode identified by a 32-byte cryptographic SHA-256 hash (`BytesN<32>`).

Upgrading `YieldStreamDistributor` replaces the contract's executable WASM hash in-place using:
```rust
env.deployer().update_current_contract_wasm(new_wasm_hash);
```
All contract instance storage (`DataKey::Admin`, `DataKey::DistributionCount`, `DataKey::Version`), persistent campaign records (`DataKey::Distribution(u32)`), and claim bitmaps/records (`DataKey::Claimed(u32, u32)`) are preserved across the transition with zero migration overhead.

### Version Tracking
The contract records monotonic version numbers in instance storage:
- Initial deployment starts at `version = 1`.
- Each invocation of `upgrade(new_wasm_hash)` increments the version: `version = current_version + 1`.
- Contract version can be inspected via the public view function:
  ```rust
  pub fn get_version(env: Env) -> u32
  ```
- An audit event is published on each successful upgrade:
  ```rust
  // Topics: ("upgrade", old_version: u32, new_version: u32)
  // Data: new_wasm_hash (BytesN<32>)
  events::contract_upgraded(&env, old_version, new_version, &new_wasm_hash);
  ```

---

## 2. Storage Key Preservation & Compatibility Guarantees

When deploying contract updates, **storage key layout compatibility must be preserved**:

| Key | Storage Type | Data Type | Compatibility Requirement |
| :--- | :--- | :--- | :--- |
| `DataKey::Admin` | Instance | `Address` | Must remain unaltered; points to current admin |
| `DataKey::DistributionCount` | Instance | `u32` | Monotonic counter of all created campaigns |
| `DataKey::Version` | Instance | `u32` | Contract logic version, incremented on upgrade |
| `DataKey::Distribution(id)` | Persistent | `Distribution` struct | Fields in `Distribution` must preserve XDR order and semantics |
| `DataKey::Claimed(id, index)` | Persistent | `bool` | Claim records for leaves |

### State Invariants Maintained
1. **Unclaimed Allocations**: Active distributions can be claimed immediately after upgrade without creator or recipient re-registration.
2. **Double-Claim Protection**: Past claim records in persistent storage remain valid, preventing replay attacks or double claims on existing distributions.
3. **Escrow Balances**: Token balances held by the contract address remain untouched.

---

## 3. Operational Upgrade Runbook

### Prerequisites
- Stellar CLI (`stellar --version >= 22.0.0`)
- Rust toolchain (`wasm32v1-none` or `wasm32-unknown-unknown`)
- Administrator secret key or multisig signer configuration

```bash
# Network parameters
export STELLAR_NETWORK="testnet" # or "mainnet"
export RPC_URL="https://soroban-testnet.stellar.org"
export NETWORK_PASSPHRASE="Test SDF Network ; September 2015"
export CONTRACT_ID="<DEPLOYED_DISTRIBUTOR_CONTRACT_ADDRESS>"
export ADMIN_IDENTITY="admin_key"
```

### Step 1: Build & Verify Deterministic WASM
Compile the release WASM using the workspace toolchain:
```bash
cargo build --target wasm32v1-none --release -p yieldstream-distributor
```
Verify optimization and calculate the byte hash:
```bash
sha256sum target/wasm32v1-none/release/yieldstream_distributor.wasm
```

### Step 2: Upload New WASM to the Ledger
Upload the new bytecode using the deployer. This registers the bytecode on-chain and outputs its 32-byte hex hash:
```bash
NEW_WASM_HASH=$(stellar contract install \
  --wasm target/wasm32v1-none/release/yieldstream_distributor.wasm \
  --source-account "$ADMIN_IDENTITY" \
  --network "$STELLAR_NETWORK")

echo "New WASM hash: $NEW_WASM_HASH"
```

### Step 3: Verify Pre-Upgrade Contract State
Query the current contract version:
```bash
CURRENT_VERSION=$(stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source-account "$ADMIN_IDENTITY" \
  --network "$STELLAR_NETWORK" \
  -- get_version)

echo "Current version: $CURRENT_VERSION"
```

### Step 4: Execute the Upgrade
Invoke the admin-restricted `upgrade` entrypoint:
```bash
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source-account "$ADMIN_IDENTITY" \
  --network "$STELLAR_NETWORK" \
  -- upgrade \
  --new_wasm_hash "$NEW_WASM_HASH"
```

### Step 5: Post-Upgrade Verification Checklist
- [ ] Confirm new version is incremented:
  ```bash
  stellar contract invoke --id "$CONTRACT_ID" --network "$STELLAR_NETWORK" -- get_version
  ```
- [ ] Verify event emission: Query ledger events with topic `upgrade`.
- [ ] Test campaign claim: Perform a test claim on an active distribution to verify state integrity and token transfer execution.
- [ ] Verify admin identity has not changed:
  ```bash
  stellar contract invoke --id "$CONTRACT_ID" --network "$STELLAR_NETWORK" -- get_admin
  ```

### Rollback Procedure
If an issue is detected post-upgrade:
1. Re-install or reference the previous known-good WASM hash (`PREVIOUS_WASM_HASH`).
2. Invoke `upgrade(PREVIOUS_WASM_HASH)` with the admin account.
3. The contract version will increment, but logic reverts to the previous bytecode.

---

## 4. Authorization Matrix

| Entrypoint | Authorized Caller | Auth Mechanism | Unauthorized Behavior |
| :--- | :--- | :--- | :--- |
| `initialize(admin)` | Anyone (once) | First caller binds `admin`; re-init fails | `ContractError::AlreadyInitialized` |
| `create_distribution(...)` | Any Creator | `creator.require_auth()` | Transaction rejected |
| `claim(...)` | Recipient | `recipient.require_auth()` | Transaction rejected |
| `cancel(distribution_id)` | Campaign Creator | `distribution.creator.require_auth()` | Transaction rejected |
| `sweep(distribution_id, to)` | Campaign Creator | `distribution.creator.require_auth()` | Transaction rejected |
| **`upgrade(new_wasm_hash)`** | **Contract Admin** | **`admin.require_auth()`** | **Transaction rejected** |
| `get_version()` | Public View | None | Open read access |

---

## 5. Written Analysis of the Trust Trade-Off

Smart contract upgradability introduces an explicit trade-off between **protocol adaptability** and **trust minimization**:

### Advantages
1. **Security Vulnerability Remediation**: Zero-day vulnerabilities in Merkle proof verification or token custody can be patched without requiring thousands of recipients and creators to migrate funds.
2. **Stellar Ecosystem Upgrades**: Accommodates Stellar Protocol transitions (e.g., Protocol 22/23 host functions, TTL extensions, new asset types).
3. **Storage & Gas Optimizations**: Enables rolling out state optimizations (such as bitmap claim packing [#13](https://github.com/yieldstream-org/yieldstream-contract/issues/13)) without breaking existing campaigns.

### Trust Risks (Centralization Vectors)
1. **Logic Replacement Power**: A compromised or malicious admin key can upload arbitrary WASM code. A rogue contract executable could:
   - Drain tokens locked in active distributions.
   - Bypass Merkle proof validation (`verify_merkle_proof`).
   - Modify campaign statuses or sweep allocations arbitrarily.
2. **Rug-Pull Perception**: Depositors (campaign creators) must trust that the admin will not replace the distributor logic while their campaigns are funded.

### Recommended Mitigation Safeguards
1. **Multi-Signature Governance**: The admin address should never be a single private key. It should be a multisig threshold account (e.g., 3-of-5 signers across independent stakeholders).
2. **Timelock / Delay Mechanism**: Major upgrades should be scheduled through an off-chain or on-chain timelock (e.g., 48 hours notice), allowing creators and recipients to verify WASM diffs or claim/sweep funds before activation.
3. **Event Transparency**: Continuous indexer monitoring on the `("upgrade", ...)` topic alerts the community the moment an upgrade transaction is committed.
4. **Ossification Horizon**: Once the protocol reaches maturity and security audits are concluded, upgradeability can be permanently revoked by setting the admin address to a dead address or removing the `upgrade` entrypoint.

---

## 6. Composition with Emergency Pause Switch (#9)

The upgrade entrypoint and the emergency pause switch ([#9](https://github.com/yieldstream-org/yieldstream-contract/issues/9)) are complementary layers of defense:

```
[ Active Vulnerability Detected ]
              │
              ▼
    1. Admin calls pause()  ────────────────► Claims & sweeps instantly halted
              │                                (User funds frozen, safe from drain)
              ▼
    2. Develop & Audit Patch
              │
              ▼
    3. stellar contract install (upload WASM)
              │
              ▼
    4. Admin calls upgrade(new_wasm_hash) ──► Bytecode replaced in-place; state intact
              │
              ▼
    5. Verify State & Tests on Mainnet
              │
              ▼
    6. Admin calls unpause() ───────────────► Claims resume normally
```

### Why Both Mechanisms Are Required
- **Pause without Upgrade**: Halts an exploit, but the contract remains stuck forever unless patched.
- **Upgrade without Pause**: Developing, compiling, verifying, and committing an upgrade takes hours or days. Without an instant pause switch, attacker transactions would drain distributions before the upgrade can be uploaded.
- **Combined Lifecycle**:
  - `pause()` provides immediate tactical defense (seconds).
  - `upgrade(...)` provides strategic resolution (hours/days).
  - The combination guarantees zero fund loss during critical incident response.
