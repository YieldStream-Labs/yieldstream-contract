import { scValToNative, xdr } from "@stellar/stellar-sdk";

export interface CreatedEvent {
  distributionId: number;
  creator: string;
  token: string;
  merkleRoot: string;
  totalAmount: bigint;
  endTime: bigint;
}

export interface ClaimedEvent {
  distributionId: number;
  recipient: string;
  index: number;
  amount: bigint;
}

export interface CancelledEvent {
  distributionId: number;
  creator: string;
  remainingAmount: bigint;
}

export interface SweptEvent {
  distributionId: number;
  creator: string;
  recipient: string;
  sweptAmount: bigint;
}

export interface UpgradedEvent {
  oldVersion: number;
  newVersion: number;
  newWasmHash: string;
}

/**
 * Decodes contract events emitted by YieldStreamDistributor.
 */
export function decodeContractEvent(
  topics: xdr.ScVal[],
  data: xdr.ScVal,
):
  | { type: "created"; event: CreatedEvent }
  | { type: "claimed"; event: ClaimedEvent }
  | { type: "cancel"; event: CancelledEvent }
  | { type: "swept"; event: SweptEvent }
  | { type: "upgrade"; event: UpgradedEvent }
  | null {
  if (topics.length === 0) return null;

  const topicName = scValToNative(topics[0])?.toString();
  if (!topicName) return null;

  switch (topicName) {
    case "created": {
      const distributionId = Number(scValToNative(topics[1]));
      const creator = scValToNative(topics[2]).toString();
      const rawData = scValToNative(data);
      return {
        type: "created",
        event: {
          distributionId,
          creator,
          token: rawData[0].toString(),
          merkleRoot: Buffer.from(rawData[1]).toString("hex"),
          totalAmount: BigInt(rawData[2]),
          endTime: BigInt(rawData[3]),
        },
      };
    }
    case "claimed": {
      const distributionId = Number(scValToNative(topics[1]));
      const recipient = scValToNative(topics[2]).toString();
      const rawData = scValToNative(data);
      return {
        type: "claimed",
        event: {
          distributionId,
          recipient,
          index: Number(rawData[0]),
          amount: BigInt(rawData[1]),
        },
      };
    }
    case "cancel": {
      const distributionId = Number(scValToNative(topics[1]));
      const creator = scValToNative(topics[2]).toString();
      const remainingAmount = BigInt(scValToNative(data));
      return {
        type: "cancel",
        event: {
          distributionId,
          creator,
          remainingAmount,
        },
      };
    }
    case "swept": {
      const distributionId = Number(scValToNative(topics[1]));
      const creator = scValToNative(topics[2]).toString();
      const rawData = scValToNative(data);
      return {
        type: "swept",
        event: {
          distributionId,
          creator,
          recipient: rawData[0].toString(),
          sweptAmount: BigInt(rawData[1]),
        },
      };
    }
    case "upgrade": {
      const oldVersion = Number(scValToNative(topics[1]));
      const newVersion = Number(scValToNative(topics[2]));
      const rawHash = scValToNative(data);
      return {
        type: "upgrade",
        event: {
          oldVersion,
          newVersion,
          newWasmHash: Buffer.from(rawHash).toString("hex"),
        },
      };
    }
    default:
      return null;
  }
}
