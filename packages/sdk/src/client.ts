import {
  Account,
  Address,
  Contract,
  Networks,
  Operation,
  Transaction,
  TransactionBuilder,
  nativeToScVal,
  rpc,
  scValToNative,
  xdr,
} from "@stellar/stellar-sdk";
import { generateTree, verifyProof } from "./merkle.js";
import type {
  ClaimManyItem,
  ClaimParams,
  ClaimProof,
  CreateCampaignParams,
  Distribution,
  DistributionStatus,
  Recipient,
  TreeResult,
  TxOptions,
  YieldStreamClientConfig,
} from "./types.js";

export const TESTNET_NETWORK_PASSPHRASE = Networks.TESTNET;
export const TESTNET_RPC_URL = "https://soroban-testnet.stellar.org";

/**
 * High-level typed client for the YieldStream Distributor protocol.
 * Wraps contract address binding, Merkle proof tree generation, view queries,
 * and Freighter / @stellar/stellar-sdk transaction construction.
 */
export class YieldStreamClient {
  readonly contractId: string;
  readonly contract: Contract;
  readonly networkPassphrase: string;
  readonly rpcUrl: string;
  private _server?: rpc.Server;

  constructor(config: YieldStreamClientConfig) {
    if (!config.contractId) {
      throw new Error("YieldStreamClient requires a contractId");
    }
    this.contractId = config.contractId;
    this.contract = new Contract(config.contractId);
    this.networkPassphrase = config.networkPassphrase ?? TESTNET_NETWORK_PASSPHRASE;
    this.rpcUrl = config.rpcUrl ?? TESTNET_RPC_URL;
  }

  get server(): rpc.Server {
    if (!this._server) {
      this._server = new rpc.Server(this.rpcUrl);
    }
    return this._server;
  }

  /**
   * Helper: Generates a Merkle tree and proof map for a campaign distribution.
   */
  generateTree(
    distributionId: number,
    recipients: readonly Recipient[],
  ): TreeResult {
    return generateTree(distributionId, recipients);
  }

  /**
   * Helper: Finds the inclusion proof for a given wallet address within a generated tree.
   */
  getProof(
    walletAddress: string,
    distributionId: number,
    tree: TreeResult,
  ): ClaimProof | undefined {
    return tree.proofs.find(
      (p) => p.recipient.toLowerCase() === walletAddress.toLowerCase(),
    );
  }

  /**
   * Helper: Verifies whether a proof is cryptographically valid against a Merkle root.
   */
  verifyProof(
    proof: readonly Uint8Array[],
    root: Uint8Array,
    leaf: Uint8Array,
  ): boolean {
    return verifyProof(proof, root, leaf);
  }

  /**
   * Builds an Operation to create and fund a new distribution campaign.
   */
  buildCreateCampaignOp(params: CreateCampaignParams): xdr.Operation {
    const rootBytes =
      typeof params.merkleRoot === "string"
        ? Buffer.from(params.merkleRoot, "hex")
        : Buffer.from(params.merkleRoot);

    if (rootBytes.length !== 32) {
      throw new Error("Merkle root must be exactly 32 bytes");
    }

    const creatorVal = Address.fromString(params.creator).toScVal();
    const tokenVal = Address.fromString(params.token).toScVal();
    const rootVal = xdr.ScVal.scvBytes(rootBytes);
    const amountVal = nativeToScVal(BigInt(params.totalAmount), { type: "i128" });
    const numVal = xdr.ScVal.scvU32(params.numRecipients);
    const endTimeVal = nativeToScVal(BigInt(params.endTime ?? 0), {
      type: "u64",
    });

    return this.contract.call(
      "create_distribution",
      creatorVal,
      tokenVal,
      rootVal,
      amountVal,
      numVal,
      endTimeVal,
    );
  }

  /**
   * Builds a full transaction for create_distribution (or createCampaign).
   */
  async buildCreateCampaignTx(
    params: CreateCampaignParams,
    options?: TxOptions,
  ): Promise<Transaction> {
    const sourceAccount =
      options?.sourceAccount ??
      (await this.server.getAccount(params.creator));

    const op = this.buildCreateCampaignOp(params);

    return new TransactionBuilder(sourceAccount, {
      fee: options?.fee?.toString() ?? "100000",
      networkPassphrase: this.networkPassphrase,
    })
      .addOperation(op)
      .setTimeout(options?.timeoutInSeconds ?? 30)
      .build();
  }

  /** Alias matching issue specification */
  createCampaign = this.buildCreateCampaignTx.bind(this);

  /**
   * Builds an Operation to claim tokens for an allocated recipient.
   */
  buildClaimOp(params: ClaimParams): xdr.Operation {
    const distIdVal = xdr.ScVal.scvU32(params.distributionId);
    const indexVal = xdr.ScVal.scvU32(params.index);
    const recipientVal = Address.fromString(params.recipient).toScVal();
    const amountVal = nativeToScVal(BigInt(params.amount), { type: "i128" });

    const proofScVals = params.proof.map((p) => {
      const bytes = typeof p === "string" ? Buffer.from(p, "hex") : Buffer.from(p);
      if (bytes.length !== 32) {
        throw new Error("Proof element must be 32 bytes");
      }
      return xdr.ScVal.scvBytes(bytes);
    });
    const proofVecVal = xdr.ScVal.scvVec(proofScVals);

    return this.contract.call(
      "claim",
      distIdVal,
      indexVal,
      recipientVal,
      amountVal,
      proofVecVal,
    );
  }

  /**
   * Builds a full claim transaction ready for Freighter signing.
   */
  async buildClaimTx(
    params: ClaimParams,
    options?: TxOptions,
  ): Promise<Transaction> {
    const sourceAccount =
      options?.sourceAccount ??
      (await this.server.getAccount(params.recipient));

    const op = this.buildClaimOp(params);

    return new TransactionBuilder(sourceAccount, {
      fee: options?.fee?.toString() ?? "100000",
      networkPassphrase: this.networkPassphrase,
    })
      .addOperation(op)
      .setTimeout(options?.timeoutInSeconds ?? 30)
      .build();
  }

  /** Alias */
  claim = this.buildClaimTx.bind(this);

  /**
   * Builds a transaction batching multiple claims in a single transaction.
   */
  async buildClaimManyTx(
    claims: readonly ClaimManyItem[],
    options?: TxOptions,
  ): Promise<Transaction> {
    if (claims.length === 0) {
      throw new Error("Claims list cannot be empty");
    }

    const firstRecipient = claims[0].recipient;
    const sourceAccount =
      options?.sourceAccount ??
      (await this.server.getAccount(firstRecipient));

    const builder = new TransactionBuilder(sourceAccount, {
      fee: options?.fee?.toString() ?? "200000",
      networkPassphrase: this.networkPassphrase,
    });

    for (const item of claims) {
      builder.addOperation(this.buildClaimOp(item));
    }

    return builder.setTimeout(options?.timeoutInSeconds ?? 30).build();
  }

  /** Alias */
  claimMany = this.buildClaimManyTx.bind(this);

  /**
   * Builds a transaction to cancel an active distribution campaign.
   */
  async buildCancelTx(
    distributionId: number,
    creator: string,
    options?: TxOptions,
  ): Promise<Transaction> {
    const sourceAccount =
      options?.sourceAccount ?? (await this.server.getAccount(creator));

    const op = this.contract.call(
      "cancel",
      xdr.ScVal.scvU32(distributionId),
    );

    return new TransactionBuilder(sourceAccount, {
      fee: options?.fee?.toString() ?? "100000",
      networkPassphrase: this.networkPassphrase,
    })
      .addOperation(op)
      .setTimeout(options?.timeoutInSeconds ?? 30)
      .build();
  }

  /**
   * Builds a transaction to sweep remaining unclaimed tokens from an expired or cancelled campaign.
   */
  async buildSweepTx(
    distributionId: number,
    creator: string,
    toAddress: string,
    options?: TxOptions,
  ): Promise<Transaction> {
    const sourceAccount =
      options?.sourceAccount ?? (await this.server.getAccount(creator));

    const toVal = Address.fromString(toAddress).toScVal();
    const op = this.contract.call(
      "sweep",
      xdr.ScVal.scvU32(distributionId),
      toVal,
    );

    return new TransactionBuilder(sourceAccount, {
      fee: options?.fee?.toString() ?? "100000",
      networkPassphrase: this.networkPassphrase,
    })
      .addOperation(op)
      .setTimeout(options?.timeoutInSeconds ?? 30)
      .build();
  }

  /**
   * Builds a transaction to upgrade contract WASM executable (admin only).
   */
  async buildUpgradeTx(
    admin: string,
    newWasmHash: Uint8Array | string,
    options?: TxOptions,
  ): Promise<Transaction> {
    const sourceAccount =
      options?.sourceAccount ?? (await this.server.getAccount(admin));

    const hashBytes =
      typeof newWasmHash === "string"
        ? Buffer.from(newWasmHash, "hex")
        : Buffer.from(newWasmHash);

    if (hashBytes.length !== 32) {
      throw new Error("Wasm hash must be 32 bytes");
    }

    const op = this.contract.call(
      "upgrade",
      xdr.ScVal.scvBytes(hashBytes),
    );

    return new TransactionBuilder(sourceAccount, {
      fee: options?.fee?.toString() ?? "100000",
      networkPassphrase: this.networkPassphrase,
    })
      .addOperation(op)
      .setTimeout(options?.timeoutInSeconds ?? 30)
      .build();
  }

  // --- Read-Only RPC Queries (Simulation) ---

  private async simulateCall(method: string, ...args: xdr.ScVal[]): Promise<any> {
    // Construct a dummy transaction to simulate through Soroban RPC
    const dummySource = new Address(
      "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
    );
    const account = new Account(dummySource.toString(), "0");

    const tx = new TransactionBuilder(account, {
      fee: "100",
      networkPassphrase: this.networkPassphrase,
    })
      .addOperation(this.contract.call(method, ...args))
      .setTimeout(30)
      .build();

    const sim = await this.server.simulateTransaction(tx);
    if (rpc.Api.isSimulationError(sim)) {
      throw new Error(`Simulation failed for ${method}: ${sim.error}`);
    }

    if (rpc.Api.isSimulationSuccess(sim) && sim.result) {
      return scValToNative(sim.result.retval);
    }

    throw new Error(`No return value from simulation of ${method}`);
  }

  /**
   * Retrieves distribution metadata and current status by ID.
   */
  async getDistribution(distributionId: number): Promise<Distribution> {
    const raw = await this.simulateCall(
      "get_distribution",
      xdr.ScVal.scvU32(distributionId),
    );

    return {
      id: Number(raw.id),
      creator: raw.creator.toString(),
      token: raw.token.toString(),
      merkleRoot: Buffer.from(raw.merkle_root).toString("hex"),
      totalAmount: BigInt(raw.total_amount),
      claimedAmount: BigInt(raw.claimed_amount),
      numRecipients: Number(raw.num_recipients),
      claimedCount: Number(raw.claimed_count),
      createdAt: BigInt(raw.created_at),
      endTime: BigInt(raw.end_time),
      status: Number(raw.status) as DistributionStatus,
    };
  }

  /**
   * Checks whether a specific leaf index in a distribution has been claimed.
   */
  async isClaimed(distributionId: number, index: number): Promise<boolean> {
    const res = await this.simulateCall(
      "is_claimed",
      xdr.ScVal.scvU32(distributionId),
      xdr.ScVal.scvU32(index),
    );
    return Boolean(res);
  }

  /**
   * Gets the total number of distributions created so far.
   */
  async getDistributionCount(): Promise<number> {
    const res = await this.simulateCall("get_distribution_count");
    return Number(res);
  }

  /**
   * Gets the contract administrator address.
   */
  async getAdmin(): Promise<string> {
    const res = await this.simulateCall("get_admin");
    return res.toString();
  }

  /**
   * Gets the current contract version.
   */
  async getVersion(): Promise<number> {
    const res = await this.simulateCall("get_version");
    return Number(res);
  }
}
