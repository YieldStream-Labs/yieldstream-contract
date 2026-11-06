import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import {
  LEAF_PREFIX,
  NODE_PREFIX,
  buildTree,
  computeLeaf,
  generateTree,
  hashPair,
  verifyProof,
} from "./merkle.js";
import { YieldStreamClient } from "./client.js";
import type { VectorFile } from "./types.js";

const here = dirname(fileURLToPath(import.meta.url));
const vectors = JSON.parse(
  readFileSync(join(here, "../test-vectors/merkle_v1.json"), "utf8"),
) as VectorFile;

describe("vector file integrity", () => {
  it("has the pinned structure", () => {
    expect(vectors.version).toMatch(/^v\d+\.\d+\.\d+$/);
    expect(vectors.trees.length).toBeGreaterThan(0);
    for (const tree of vectors.trees) {
      expect(tree.root_hex).toMatch(/^[0-9a-f]{64}$/);
      expect(tree.num_recipients).toBeGreaterThan(0);
      expect(tree.claims.length).toBe(tree.num_recipients);
      for (const claim of tree.claims) {
        expect(claim.leaf_hex).toMatch(/^[0-9a-f]{64}$/);
        expect(claim.proof_hex.length).toBeLessThanOrEqual(32);
      }
    }
  });
});

describe("domain separation constants", () => {
  it("match the contract (merkle.rs)", () => {
    expect(LEAF_PREFIX).toBe(0x00);
    expect(NODE_PREFIX).toBe(0x01);
  });
});

describe("rust conformance: computeLeaf == merkle_v1.json leaves", () => {
  it("every vector leaf hash matches", () => {
    for (const tree of vectors.trees) {
      for (const claim of tree.claims) {
        const leaf = computeLeaf(
          tree.distribution_id,
          claim.index,
          claim.recipient,
          BigInt(claim.amount),
        );
        expect(Buffer.from(leaf).toString("hex")).toBe(claim.leaf_hex);
      }
    }
  });
});

describe("rust conformance: buildTree reproduces roots and proofs", () => {
  it("roots match and proofs verify", () => {
    for (const tree of vectors.trees) {
      const leaves = tree.claims.map((c) =>
        computeLeaf(
          tree.distribution_id,
          c.index,
          c.recipient,
          BigInt(c.amount),
        ),
      );
      const built = buildTree(leaves);
      expect(Buffer.from(built.root).toString("hex")).toBe(tree.root_hex);

      for (let i = 0; i < tree.claims.length; i++) {
        const expectedProofHex = tree.claims[i].proof_hex;
        const actualProofHex = built.proofs[i].map((p) =>
          Buffer.from(p).toString("hex"),
        );
        expect(actualProofHex).toEqual(expectedProofHex);

        const verified = verifyProof(
          built.proofs[i],
          built.root,
          leaves[i],
        );
        expect(verified).toBe(true);
      }
    }
  });
});

describe("high-level generateTree API", () => {
  it("produces identical roots and verified proofs from recipient items", () => {
    for (const tree of vectors.trees) {
      const recipients = tree.claims.map((c) => ({
        index: c.index,
        recipient: c.recipient,
        amount: c.amount,
      }));

      const res = generateTree(tree.distribution_id, recipients);
      expect(res.rootHex).toBe(tree.root_hex);
      expect(res.numRecipients).toBe(tree.num_recipients);
      expect(res.totalAmount).toBe(BigInt(tree.total_amount));

      for (let i = 0; i < tree.claims.length; i++) {
        expect(res.proofs[i].leafHex).toBe(tree.claims[i].leaf_hex);
        expect(res.proofs[i].proofHex).toEqual(tree.claims[i].proof_hex);
        expect(
          verifyProof(res.proofs[i].proof, res.root, res.proofs[i].leaf),
        ).toBe(true);
      }
    }
  });
});

describe("YieldStreamClient typed client", () => {
  const dummyContractId =
    "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAMDR4";
  const client = new YieldStreamClient({
    contractId: dummyContractId,
  });

  it("binds contract ID and network", () => {
    expect(client.contractId).toBe(dummyContractId);
    expect(client.networkPassphrase).toContain("Test SDF Network");
  });

  it("builds createCampaign operation", () => {
    const tree = vectors.trees[0];
    const op = client.buildCreateCampaignOp({
      creator: tree.claims[0].recipient,
      token: "CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC",
      merkleRoot: tree.root_hex,
      totalAmount: tree.total_amount,
      numRecipients: tree.num_recipients,
      endTime: 1700000000,
    });
    const body = typeof (op as any).body === "function" ? (op as any).body() : (op as any).body;
    const bodyType = body?.switch ? body.switch().name : body?.type;
    expect(bodyType).toBe("invokeHostFunction");
  });

  it("builds claim operation", () => {
    const tree = vectors.trees[0];
    const claim = tree.claims[0];
    const op = client.buildClaimOp({
      distributionId: tree.distribution_id,
      index: claim.index,
      recipient: claim.recipient,
      amount: claim.amount,
      proof: claim.proof_hex,
    });
    const body = typeof (op as any).body === "function" ? (op as any).body() : (op as any).body;
    const bodyType = body?.switch ? body.switch().name : body?.type;
    expect(bodyType).toBe("invokeHostFunction");
  });

  it("finds proof for wallet in tree", () => {
    const tree = vectors.trees[0];
    const recipients = tree.claims.map((c) => ({
      address: c.recipient,
      amount: c.amount,
    }));
    const generated = client.generateTree(tree.distribution_id, recipients);

    const target = tree.claims[1].recipient;
    const proof = client.getProof(target, tree.distribution_id, generated);
    expect(proof).toBeDefined();
    expect(proof?.recipient).toBe(target);
    expect(proof?.index).toBe(1);
    expect(proof?.leafHex).toBe(tree.claims[1].leaf_hex);
  });
});
