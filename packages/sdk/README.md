<div align="center">
<img src="https://raw.githubusercontent.com/yieldstream-org/yieldstream-app/main/app/icon.svg" alt="YieldStream logo" width="80" height="80" />

# @yieldstream/sdk

[![CI](https://github.com/yieldstream-org/yieldstream-sdk/actions/workflows/ci.yml/badge.svg)](https://github.com/yieldstream-org/yieldstream-sdk/actions/workflows/ci.yml)
[![Target](https://img.shields.io/badge/node-ES2022-informational)](https://nodejs.org)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

[Contract](https://github.com/yieldstream-org/yieldstream-contract) · [App](https://github.com/yieldstream-org/yieldstream-app) · [Docs](https://yieldstream-protocol.vercel.app/docs) · [Issues](https://github.com/yieldstream-org/yieldstream-sdk/issues)

</div>

TypeScript client and Merkle proof tooling for the [YieldStream distributor](https://github.com/yieldstream-org/yieldstream-contract) — a multi-tenant Merkle claim protocol on Stellar/Soroban.

---

## Features

- **Byte-Identical Merkle Engine**: `computeLeaf`, `hashPair`, and `buildTree` match the Rust Soroban contract's XDR-encoded leaf hashing and RFC 6962 domain separation byte-for-byte (validated against `test-vectors/merkle_v1.json`).
- **Typed Protocol Client**: `YieldStreamClient` with contract address binding, RPC simulation view queries (`getDistribution`, `isClaimed`, `getVersion`, `getAdmin`), and transaction builders.
- **Freighter Wallet Ready**: Built for `@stellar/stellar-sdk` and `@stellar/freighter-api` integration for web frontends.
- **Lifecycle Event Decoding**: Decodes contract events (`created`, `claimed`, `cancel`, `swept`, `upgrade`).

---

## Installation

```bash
npm install @yieldstream/sdk @stellar/stellar-sdk
```

---

## Quickstart: Copy-Paste Example

### 1. Generating a Campaign Tree & Proofs (Creator / Issuer)

```typescript
import { generateTree, YieldStreamClient } from "@yieldstream/sdk";

// Define campaign recipient allocations
const recipients = [
  { address: "GBZXN7PIRZGNMHGA7MUUUF4GWPY5AYPV6LY4UV2GL6VJGIQRXFDNMADI", amount: 100_000_000n }, // 10.0 tokens (7 decimals)
  { address: "GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWN7", amount: 250_000_000n }, // 25.0 tokens
  { address: "GBLA4V54TC6K5544SZB7F24JYCJQ3B44673B3U6I4B624CYE2Z2Z2ABC", amount: 500_000_000n }, // 50.0 tokens
];

// Build the deterministic Merkle tree for campaign ID 1
const distributionId = 1;
const tree = generateTree(distributionId, recipients);

console.log("Root Hex:", tree.rootHex);
console.log("Total tokens:", tree.totalAmount);

// Find the claim proof for a specific wallet
const targetWallet = "GBZXN7PIRZGNMHGA7MUUUF4GWPY5AYPV6LY4UV2GL6VJGIQRXFDNMADI";
const userClaim = tree.proofs.find((p) => p.recipient === targetWallet);

console.log("User Leaf Hash:", userClaim?.leafHex);
console.log("Inclusion Proof:", userClaim?.proofHex);
```

### 2. Creating a Campaign on Stellar / Soroban

```typescript
import { YieldStreamClient } from "@yieldstream/sdk";

const client = new YieldStreamClient({
  contractId: "CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC",
  rpcUrl: "https://soroban-testnet.stellar.org",
});

// Build transaction to create and fund the distribution
const tx = await client.buildCreateCampaignTx({
  creator: "GBZXN7PIRZGNMHGA7MUUUF4GWPY5AYPV6LY4UV2GL6VJGIQRXFDNMADI",
  token: "CBIELTK6YBZJU5UP2WWQEUCYKLPU6AUNZ2BQ4WWUIE3USNONGSBEQECU",
  merkleRoot: tree.root,
  totalAmount: tree.totalAmount,
  numRecipients: tree.numRecipients,
  endTime: 1750000000n, // Unix timestamp (0 for no expiry)
});

// Sign with Freighter and submit:
// const signedTxXdr = await signTransaction(tx.toXDR(), { networkPassphrase: client.networkPassphrase });
// const result = await client.server.sendTransaction(new Transaction(signedTxXdr, client.networkPassphrase));
```

### 3. Claiming Allocations (Frontend / User)

```typescript
import { YieldStreamClient } from "@yieldstream/sdk";

const client = new YieldStreamClient({
  contractId: "CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC",
});

// Check if wallet already claimed
const alreadyClaimed = await client.isClaimed(distributionId, userClaim.index);
if (alreadyClaimed) {
  console.log("Allocation already claimed!");
} else {
  // Build claim transaction
  const claimTx = await client.buildClaimTx({
    distributionId,
    index: userClaim.index,
    recipient: userClaim.recipient,
    amount: userClaim.amount,
    proof: userClaim.proofHex,
  });

  // Sign via Freighter extension:
  // import { signTransaction } from "@stellar/freighter-api";
  // const signed = await signTransaction(claimTx.toXDR(), { networkPassphrase: client.networkPassphrase });
}
```

---

## API Reference

### Merkle Core (`@yieldstream/sdk/merkle`)
- `computeLeaf(distributionId, index, recipientAddress, amount): Uint8Array` — Generates canonical leaf hash.
- `hashPair(a, b): Uint8Array` — Sibling sorted-pair hasher.
- `buildTree(leaves): { root, proofs }` — Power-of-two padded Merkle tree constructor.
- `generateTree(distributionId, recipients): TreeResult` — End-to-end tree generator with hex outputs.
- `verifyProof(proof, root, leaf): boolean` — Off-chain inclusion verification.

### Client (`YieldStreamClient`)
- `buildCreateCampaignTx(params, options): Promise<Transaction>`
- `buildClaimTx(params, options): Promise<Transaction>`
- `buildClaimManyTx(claims, options): Promise<Transaction>`
- `buildCancelTx(distributionId, creator, options): Promise<Transaction>`
- `buildSweepTx(distributionId, creator, toAddress, options): Promise<Transaction>`
- `buildUpgradeTx(admin, newWasmHash, options): Promise<Transaction>`
- `getDistribution(distributionId): Promise<Distribution>`
- `isClaimed(distributionId, index): Promise<boolean>`
- `getVersion(): Promise<number>`
- `getAdmin(): Promise<string>`

---

## Development & Testing

```bash
npm install
npm run typecheck   # Typecheck against TypeScript strict mode
npm run build       # Build distribution bundle with declarations & sourcemaps
npm test            # Run Vitest test suite including Rust cross-language vectors
```

---

## License

Dual licensed under MIT OR Apache-2.0, at your option.
