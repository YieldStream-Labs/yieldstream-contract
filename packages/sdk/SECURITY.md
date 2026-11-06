# Security Policy

## Reporting a vulnerability

Report security issues in the YieldStream SDK **privately** via
[GitHub private vulnerability disclosure](https://github.com/yieldstream-org/yieldstream-sdk/security/advisories/new).
Please do not open a public issue.

## Why this repo matters for fund safety

The SDK builds the Merkle tree that the contract later verifies claims against.
A defect here does not show up as a crash — it shows up as an issuer escrowing
a root that does not commit to the list they intended, or as proofs that fail
against a correctly committed root. Both are silent and both cost real money.

The highest-value bug in this repo is a hashing or tree-building path that
agrees with itself but disagrees with the contract.

## Scope

- Leaf encoding: domain prefix, XDR field order and widths, `i128` amount
  handling, `Address` encoding
- Node hashing: sorted-pair ordering, domain prefix
- Proof generation: path selection, level indexing, odd-count tree promotion
- Conformance drift between the TypeScript implementation and
  `yieldstream-contract/src/merkle.rs`

## Status

The hashing core is **not yet implemented** — `computeLeaf` and `buildTree`
throw, and the cross-language conformance suites are skipped pending the
contract-format work tracked in issues #1 and #2. Treat any local
implementation as pre-release until the conformance tests run green.

## Response window

Acknowledge within 72 hours; fix or mitigation plan within 14 days for anything
that can produce a wrong root.
