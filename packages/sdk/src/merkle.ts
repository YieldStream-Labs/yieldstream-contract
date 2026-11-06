import { createHash } from "node:crypto";
import { Address, nativeToScVal, xdr } from "@stellar/stellar-sdk";
import type {
  ClaimItem,
  ClaimProof,
  Hash32,
  Recipient,
  TreeResult,
} from "./types.js";

/** RFC 6962 domain-separation byte for leaf hashes. Must match merkle.rs. */
export const LEAF_PREFIX = 0x00;
/** RFC 6962 domain-separation byte for internal node hashes. Must match merkle.rs. */
export const NODE_PREFIX = 0x01;

/**
 * Computes SHA-256 hash over arbitrary byte input.
 */
export function sha256(data: Uint8Array): Uint8Array {
  return createHash("sha256").update(data).digest();
}

/**
 * Hashes two sibling nodes with sorted pair ordering (OpenZeppelin / RFC 6962 pattern).
 *
 * Format: SHA256( 0x01 || min(a, b) || max(a, b) )
 */
export function hashPair(a: Uint8Array, b: Uint8Array): Uint8Array {
  const bufA = Buffer.from(a);
  const bufB = Buffer.from(b);
  const [min, max] = Buffer.compare(bufA, bufB) <= 0 ? [bufA, bufB] : [bufB, bufA];
  return sha256(Buffer.concat([Buffer.from([NODE_PREFIX]), min, max]));
}

/**
 * Compute the canonical YieldStream leaf hash byte-identical to the Rust contract.
 *
 * Format (pinned by merkle.rs):
 *   SHA-256( 0x00 || XDR(distribution_id:u32) || XDR(index:u32)
 *                 || XDR(recipient:Address) || XDR(amount:i128) )
 */
export function computeLeaf(
  distributionId: number,
  index: number,
  recipientAddress: string,
  amount: bigint | string | number,
): Hash32 {
  const prefix = Buffer.from([LEAF_PREFIX]);
  const idXdr = xdr.ScVal.scvU32(distributionId).toXDR();
  const indexXdr = xdr.ScVal.scvU32(index).toXDR();
  const addrXdr = Address.fromString(recipientAddress).toScVal().toXDR();
  const amtXdr = nativeToScVal(BigInt(amount), { type: "i128" }).toXDR();

  const preimage = Buffer.concat([prefix, idXdr, indexXdr, addrXdr, amtXdr]);
  return Uint8Array.from(sha256(preimage));
}

/**
 * Returns the smallest power of 2 greater than or equal to n.
 */
function nextPowerOfTwo(n: number): number {
  if (n <= 1) return 1;
  let p = 1;
  while (p < n) {
    p <<= 1;
  }
  return p;
}

/**
 * Verifies whether a given leaf belongs to the Merkle tree with the specified root.
 */
export function verifyProof(
  proof: readonly Uint8Array[],
  root: Uint8Array,
  leaf: Uint8Array,
): boolean {
  let current = leaf;
  for (const sibling of proof) {
    current = hashPair(current, sibling);
  }
  return Buffer.compare(Buffer.from(current), Buffer.from(root)) === 0;
}

/**
 * Build the tree over canonical leaves and return root + per-index proofs.
 * Internal nodes: SHA-256(0x01 || min(a,b) || max(a,b)) — sorted pairs.
 * Pads leaves to the next power of 2 by duplicating the last leaf (matching testutils).
 */
export function buildTree(leaves: readonly Hash32[]): {
  root: Hash32;
  proofs: Hash32[][];
} {
  const n = leaves.length;
  if (n === 0) {
    throw new Error("Cannot build Merkle tree from empty leaves");
  }

  const targetSize = nextPowerOfTwo(n);
  let currentLevel: Uint8Array[] = [...leaves];
  while (currentLevel.length < targetSize) {
    currentLevel.push(currentLevel[currentLevel.length - 1]);
  }

  const treeLevels: Uint8Array[][] = [currentLevel];
  while (currentLevel.length > 1) {
    const nextLevel: Uint8Array[] = [];
    for (let i = 0; i < currentLevel.length; i += 2) {
      nextLevel.push(hashPair(currentLevel[i], currentLevel[i + 1]));
    }
    treeLevels.push(nextLevel);
    currentLevel = nextLevel;
  }

  const root = currentLevel[0];

  // Generate proofs for the original items
  const proofs: Hash32[][] = [];
  for (let i = 0; i < n; i++) {
    const proof: Hash32[] = [];
    let idx = i;
    for (let lvl = 0; lvl < treeLevels.length - 1; lvl++) {
      const siblingIdx = idx % 2 === 0 ? idx + 1 : idx - 1;
      proof.push(treeLevels[lvl][siblingIdx]);
      idx = Math.floor(idx / 2);
    }
    proofs.push(proof);
  }

  return { root, proofs };
}

/**
 * Generates a full canonical Merkle tree, root, and inclusion proofs from a recipient list.
 *
 * @param distributionId Sequential distribution ID for domain separation.
 * @param recipients Array of recipient objects { address, amount } or { index, recipient, amount }.
 */
export function generateTree(
  distributionId: number,
  recipients: readonly (Recipient | ClaimItem)[],
): TreeResult {
  if (recipients.length === 0) {
    throw new Error("Recipient list cannot be empty");
  }

  let totalAmount = 0n;
  const items: ClaimItem[] = recipients.map((r, i) => {
    const addr = "recipient" in r ? r.recipient : r.address;
    const amountBig = BigInt(r.amount);
    totalAmount += amountBig;
    return {
      index: "index" in r ? r.index : i,
      recipient: addr,
      amount: amountBig,
    };
  });

  const leaves = items.map((item) =>
    computeLeaf(distributionId, item.index, item.recipient, item.amount),
  );

  const { root, proofs } = buildTree(leaves);

  const claimProofs: ClaimProof[] = items.map((item, i) => {
    const proof = proofs[i];
    return {
      index: item.index,
      recipient: item.recipient,
      amount: BigInt(item.amount),
      leaf: leaves[i],
      leafHex: Buffer.from(leaves[i]).toString("hex"),
      proof,
      proofHex: proof.map((p) => Buffer.from(p).toString("hex")),
    };
  });

  return {
    root,
    rootHex: Buffer.from(root).toString("hex"),
    leaves,
    proofs: claimProofs,
    totalAmount,
    numRecipients: items.length,
  };
}
