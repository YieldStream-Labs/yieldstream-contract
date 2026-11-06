# Security Policy

## Reporting a vulnerability

If you find a security issue in the YieldStream distributor contract or its
tooling, please report it **privately**:

- Use [GitHub private vulnerability disclosure](https://github.com/yieldstream-org/yieldstream-contract/security/advisories/new), or
- Open a draft security advisory against the repository.

Please do **not** open a public issue for vulnerabilities.

## Audit status

**This contract has not been externally audited.** It is deployed to Stellar
testnet only. The safeguards that exist are in-repo: 21 tests, RFC 6962
domain separation with sorted-pair proofs, deterministic test vectors pinned in
`test-vectors/merkle_v1.json`, and CI running `cargo fmt --check`,
`clippy -D warnings`, and the full suite on every push.

Do not custody mainnet funds with this contract. Known open fund-safety issues
are tracked publicly in the issue backlog rather than hidden.

## Scope

Security-relevant surfaces include:

- Merkle proof verification (`src/merkle.rs`) — proof malleability,
  second-preimage, leaf/internal domain separation (RFC 6962)
- Claim accounting and double-claim prevention (`src/storage.rs`, `src/lib.rs`)
- Token custody, cancellation and sweep paths (fund-loss vectors)
- Multi-tenant isolation — one tenant must never affect another's state
- TTL / ledger-footprint correctness under adversarial interaction patterns

## Response window

We aim to acknowledge reports within **72 hours** and ship a fix or mitigation
plan within **14 days** for confirmed issues affecting funds.

## Disclosure

We coordinate disclosure with the reporter and will credit findings in the
advisory once fixed, at the reporter's preference.
