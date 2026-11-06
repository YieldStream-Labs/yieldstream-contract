export type Hash32 = Uint8Array;

export interface Recipient {
  /** Stellar address (G... for accounts, C... for contracts) */
  address: string;
  /** Token amount in base units / contract scale (bigint, string, or number) */
  amount: bigint | string | number;
}

export interface ClaimItem {
  index: number;
  recipient: string;
  amount: bigint | string | number;
}

export interface ClaimProof {
  index: number;
  recipient: string;
  amount: bigint;
  leaf: Uint8Array;
  leafHex: string;
  proof: Uint8Array[];
  proofHex: string[];
}

export interface TreeResult {
  root: Uint8Array;
  rootHex: string;
  leaves: Uint8Array[];
  proofs: ClaimProof[];
  totalAmount: bigint;
  numRecipients: number;
}

export enum DistributionStatus {
  Active = 0,
  Cancelled = 1,
  Completed = 2,
  Expired = 3,
}

export interface Distribution {
  id: number;
  creator: string;
  token: string;
  merkleRoot: string;
  totalAmount: bigint;
  claimedAmount: bigint;
  numRecipients: number;
  claimedCount: number;
  createdAt: bigint;
  endTime: bigint;
  status: DistributionStatus;
}

export interface YieldStreamClientConfig {
  /** Contract address on Stellar (starts with C...) */
  contractId: string;
  /** Stellar network passphrase (defaults to Testnet passphrase) */
  networkPassphrase?: string;
  /** Horizon / Soroban RPC URL */
  rpcUrl?: string;
}

export interface CreateCampaignParams {
  creator: string;
  token: string;
  merkleRoot: Uint8Array | string;
  totalAmount: bigint | string | number;
  numRecipients: number;
  endTime?: bigint | number;
}

export interface ClaimParams {
  distributionId: number;
  index: number;
  recipient: string;
  amount: bigint | string | number;
  proof: (Uint8Array | string)[];
}

export interface ClaimManyItem {
  distributionId: number;
  index: number;
  recipient: string;
  amount: bigint | string | number;
  proof: (Uint8Array | string)[];
}

export interface TxOptions {
  fee?: number | string;
  timeoutInSeconds?: number;
  sourceAccount?: any;
}

export interface ClaimVector {
  index: number;
  recipient: string;
  amount: number;
  leaf_hex: string;
  proof_hex: string[];
}

export interface TreeVector {
  name: string;
  distribution_id: number;
  total_amount: number;
  num_recipients: number;
  root_hex: string;
  claims: ClaimVector[];
}

export interface VectorFile {
  version: string;
  description: string;
  trees: TreeVector[];
}
