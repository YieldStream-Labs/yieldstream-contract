# YieldStream — Contributor Issue Backlog

Complexity tiers follow the Drips Wave scale: **100** (approachable), **150**
(moderate), **200** (deep work). Labels used below map to GitHub labels:
`good-first-issue`, `docs`, `contract`, `tooling`, `tests`, `security`,
`gas-footprint`, plus a tier label (`tier-100`, `tier-150`, `tier-200`).

This document is the staging area. Each entry becomes a real GitHub issue with
full acceptance criteria when we publish.

---

## Tier 100 — good-first-issue candidates

### 1. Docs: end-to-end testnet walkthrough with the Stellar CLI
- **Labels:** good-first-issue, docs, tier-100
- **Context:** `scripts/deploy.sh` exists but there is no narrative guide for a
  newcomer: install Stellar CLI → fund testnet account → build WASM → deploy →
  `initialize` → create a 4-recipient distribution from `test-vectors/merkle_v1.json`
  → claim with a proof → verify balances.
- **Goal:** `docs/GETTING_STARTED.md` a beginner can follow with zero prior
  Soroban experience.
- **Acceptance:** every command copy-pasteable; expected outputs shown;
  troubleshooting section for common errors (missing identity, unfunded account,
  TTL).

### 2. Contract: `get_distributions_by_creator` paginated view
- **Labels:** good-first-issue, contract, tier-100
- **Context:** tenants cannot enumerate their own campaigns; callers must scan
  IDs blindly (`storage.rs` keys are ID-based).
- **Goal:** secondary index `DataKey::CreatorDistributions(Address)` → vector of
  IDs, new view `get_distributions_by_creator(creator, offset, limit)`.
- **Acceptance:** correct after create/cancel/sweep; bounded limit (≤ 20);
  test covering multi-tenant isolation; TTL bumps on read.

### 3. Tests: reach every `ContractError` variant
- **Labels:** good-first-issue, tests, tier-100
- **Context:** `errors.rs` defines variants; the suite exercises most but not
  provably all (`ArithmeticOverflow`, `NotInitialized` paths are hard to hit).
- **Goal:** add `assert!(matches!(env.panic_error(), ...))`-style or
  `Result::Err`-equality tests so every variant is asserted at least once; add
  a checklist table to `docs/TESTING.md`.
- **Acceptance:** `cargo test` shows one test per variant; table maps
  variant → test file.

### 4. Docs: event schema reference
- **Labels:** good-first-issue, docs, tier-100
- **Context:** `events.rs` emits 4 structured events; indexers have no
  documented topics/data layout.
- **Goal:** `docs/EVENTS.md` with topic lists, data types, XDR shapes, and one
  worked Horizon/`soroban events` parsing example per event.
- **Acceptance:** examples run against a testnet deployment guide reference.

### 5. Tooling: `#![doc]` lint enforcement + missing doc comments
- **Labels:** good-first-issue, contract, docs, tier-100
- **Goal:** add `#![deny(missing_docs)]` (or clippy `missing_docs`) and fill any
  gaps; keep doc comments factual — no marketing tone.
- **Acceptance:** CI green with the lint on; public items documented.

---

## Tier 150 — moderate, self-contained

### 6. Security: bound Merkle proof length
- **Labels:** security, contract, tests, tier-150
- **Context:** `merkle::verify_merkle_proof` loops over an attacker-supplied
  `Vec<BytesN<32>>` with no length cap. Deep proofs burn instructions; combined
  with `create_distribution`'s unrestricted `num_recipients`, a tenant can craft
  campaigns that make verification paths pathological.
- **Goal:** enforce `MAX_PROOF_LEN` (32 → supports up to 2^32 leaves) in
  `claim` and `verify_proof`; add tests for at-limit and over-limit proofs.
- **Acceptance:** over-long proofs fail with `InvalidProof` (or new error);
  vectors still pass; instruction-count diff recorded in PR.

### 7. Contract: input validation on `create_distribution`
- **Labels:** contract, security, tier-150
- **Context:** `num_recipients` accepts any `u32`; all-zero `merkle_root`
  accepted; no sanity bound between `num_recipients` and `total_amount`; token
  arg not checked to be a contract (classic assets fail later, confusingly).
- **Goal:** reject zero root; cap `num_recipients` at a protocol limit (e.g.
  2^20); require `token` to be a deployed contract address
  (`env_ledger()` check) with a clear error variant.
- **Acceptance:** new error variants + tests per rejection path; README
  interface table updated.

### 8. Contract: preserve accounting truth in `sweep`
- **Labels:** contract, tier-150
- **Context:** `sweep` sets `claimed_amount = total_amount` to mark completion,
  corrupting real claim statistics (see `lib.rs` sweep path).
- **Goal:** add `swept_amount` field (backward-compatible storage versioning or
  migration note); keep `claimed_amount` truthful; event carries both values.
- **Acceptance:** stats correct after partial-claim-then-sweep; test asserting
  `claimed_amount` unchanged by sweep; migration handled or documented.

### 9. Contract: emergency pause switch
- **Labels:** security, contract, tier-150
- **Context:** `initialize(admin)` stores an admin that is never used again.
  If a bug is discovered live, there is no way to halt claims.
- **Goal:** admin-only `set_paused(bool)`; `claim` reverts with
  `DistributionPaused` while paused; `create_distribution` also gated; pause
  does NOT block `cancel`/`sweep` (creators must always be able to exit); event
  + view `is_paused`.
- **Acceptance:** full auth tests (non-admin rejected); interaction matrix
  documented; tests for pause + expiry edge (`end_time == 0` campaigns).

### 10. Tests: fuzz + property-based harness for proof verification
- **Labels:** tests, security, tooling, tier-150
- **Goal:** `cargo-hafnium` (or `proptest` over a mockable hasher) fuzzing
  `verify_merkle_proof`: no proof should verify for a tampered leaf, wrong
  root, or mutated sibling; domain-separation bytes respected; compare against
  an independent reference tree implementation in test code.
- **Acceptance:** fuzz target checked into `fuzz/` or test suite with
  deterministic seed; findings (if any) turned into regression tests.

### 11. CI: optimized WASM size budget + release artifacts
- **Labels:** tooling, tier-150
- **Goal:** extend CI: run `stellar contract optimize`, assert `.wasm` size
  under a documented budget (e.g. < 20 KB), attach optimized artifact to a
  `v0.1.x` release; optional: `cargo-ledger` footprint report on PRs.
- **Acceptance:** workflow green on current code; budget documented in CI
  comment; release workflow only fires on tags.

### 12. Contract: native XLM (classic asset) distribution — or a kind error
- **Labels:** contract, tier-150
- **Context:** `token::Client` only works for Soroban ERC-20-style tokens;
  classic assets (XLM, issued tokens) cannot be passed, failing late/confusing.
- **Goal:** decide + implement: either support classic assets via
  `Stellar::Client::transfer` branch, or add explicit rejection with a
  `ClassicAssetUnsupported` variant and docs pointer to wrapped assets
  (e.g. wrapped XLM on Stellar).
- **Acceptance:** chosen path fully tested; README interface table notes the
  limitation or capability.

---

## Tier 200 — deep, high-impact

### 13. Gas/footprint: bitmap claim storage (flagship optimization)
- **Labels:** gas-footprint, contract, storage, tier-200
- **Context:** one persistent ledger entry per claimed leaf
  (`DataKey::Claimed(id, index)`) — a 10,000-recipient campaign costs 10,000
  entries of rent, paid across claim transactions. This is THE scaling problem
  of every naive distributor.
- **Goal:** pack claimed flags into `u128`/bytes words (256 leaves per entry),
  ~32-40 entries for 10k recipients. Include: read-modify-write in `claim`,
  atomicity under the same-transaction failure paths, index math tests at
  word boundaries (0, 255, 256, 4095, 4096), migration strategy or clean
  pre-1.0 cutover, and a benchmark note (rent before/after) in
  `docs/STORAGE.md`.
- **Acceptance:** all 21 existing tests pass or are consciously updated; new
  boundary tests; `soroban-budget-assert`-style footprint assertions; written
  comparison of ledger-entry counts in PR description.

### 14. Resilience: TTL-expiry fund orphaning — recovery design
- **Labels:** security, storage, contract, tier-200
- **Context:** persistent entries live ~120 days after last touch
  (`storage.rs` bumps). A long campaign whose record expires is unfetchable by
  `get_distribution`, yet the contract still custodies the tokens → funds
  orphaned with no recovery path. `is_claimed` returning `false` on an expired
  claimed-word would ALSO re-open double claims if only some entries expired.
- **Goal:** design + implement protection: bound `end_time` relative to
  bumpable horizon, an `extend_campaign` anyone-pays entrypoint to bump TTLs,
  and/or a documented `rescue` flow. Write `docs/TTL.md` analyzing the failure
  modes precisely.
- **Acceptance:** design doc reviewed by maintainer in the issue thread first;
  implemented guard(s); test that fast-forwards ledgers past TTL to prove the
  fix.

### 15. Feature: `claim_many` — batched claims for one recipient
- **Labels:** contract, gas-footprint, tier-200
- **Context:** a user holding many leaves across campaigns pays a full base
  transaction cost per claim.
- **Goal:** `claim_many(Vec<Claim>)` reusing one auth; per-claim error
  isolation decision (all-or-nothing is required: partial state on revert is
  unsafe — document why); bounded batch size tied to proof budget from #6.
- **Acceptance:** amortized-cost measurement in PR; double-claim regression
  tests inside batch; batch-size cap enforced; docs/README updated.

### 16. Tooling: `yieldstream-tree` — proof generator CLI
- **Labels:** tooling, rust, tier-200
- **Goal:** Rust CLI: recipients CSV (`address,amount`) → canonical tree
  (exact leaf format of `merkle.rs`, including XDR encodings and domain
  separation) → `root.json` + `proofs.json` matching `test-vectors/merkle_v1.json`.
  Output must round-trip through `verify_proof` on a testnet deployment
  (CI integration optional).
- **Acceptance:** golden-file test against the committed vectors; `--verify`
  mode; README quickstart updated to use the CLI; `cargo install` ready.

### 17. Ecosystem: TypeScript SDK `@yieldstream/sdk`
- **Labels:** tooling, typescript, tier-200
- **Goal:** typed client for the distributor: address binding, `createCampaign`,
  `generateTree` (port of #16 leaf hashing for JS), `getProof(wallet,
  distributionId)`, `claim`/`claimMany` transaction builders for `@stellar/sdk`
  + Freighter integration; consumed by the web app.
- **Acceptance:** package with vitest suite proving JS leaf hashes == Rust
  leaves (cross-vector conformance against `merkle_v1.json`); docs with
  copy-paste example; published under an org npm scope.

### 18. Lifecycle: contract upgradability (`upgrade` entrypoint)
- **Labels:** contract, security, tier-200
- **Goal:** admin-only `upgrade(new_wasm_hash)` via
  `env.deployer().upload_contract_wasm` + `update` pattern with salt/version
  tracking, post-upgrade state compatibility (all storage keys preserved),
  `get_version` view, and a written upgrade runbook in `docs/UPGRADING.md`.
- **Acceptance:** test deploying a v2 WASM in testutils and claiming with v1
  state intact; auth matrix; explicit documentation of the trust trade-off
  (admin can replace logic) and how #9 pause composes with it.

---

## Suggested first-week wave

For the Wave program application we want: 2× tier-100, 1× tier-150, 1× tier-200
visible as `good-first-issue`-friendly and fully specced. Recommended: #1, #2,
#9, #13 — a docs starter, a bounded feature, a security feature, and the
flagship optimization that gives the project its reputation.
