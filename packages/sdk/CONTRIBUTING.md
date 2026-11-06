# Contributing to @yieldstream/sdk

## Cardinal rule: byte-identical to the Rust contract

Every hash this SDK produces MUST equal `yieldstream-contract`'s `merkle.rs`
output — same domain separation (`0x00` leaf / `0x01` node), same XDR field
encodings, sorted sibling pairs on internal nodes. The proof of "done" is the
skipped conformance suite in `src/conformance.test.ts` against
`test-vectors/merkle_v1.json`.

- `test-vectors/merkle_v1.json` is vendored from the contract repo. Do not
  hand-edit. If it changed upstream, sync it and note the source commit.
- PRs must pass `npm run typecheck` and `npm test`.
- New API surface gets unit tests, not just conformance.

## Setup

```bash
npm install
npm test          # vitest
npm run typecheck # tsc --noEmit
```

## Good starting points

Issue #1 (leaf hashing) and #2 (tree builder) are specced with acceptance
criteria; comment to be assigned before starting.

## License

Contributions are dual-licensed MIT / Apache-2.0 at the recipient's option.

## Code of conduct

Participation in this repository and every other `yieldstream-org`
space is governed by [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md). Reporting
concerns is described there.
