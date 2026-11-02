export type Network = "mainnet" | "testnet";

export interface VesselConfig {
  network: Network;
  nftContract: string;
  marketplaceContract: string;
  verifierContract?: string;
}

export interface RpcResult<T> {
  result: T;
  blockHeight: number;
}

const RPC_URLS: Record<Network, string> = {
  mainnet: "https://rpc.mainnet.near.org",
  testnet: "https://rpc.testnet.near.org",
};

function assertAccountId(accountId: string): void {
  if (!/^[a-z0-9._-]+$/.test(accountId) || accountId.length > 64) {
    throw new Error(`Invalid NEAR account ID: ${accountId}`);
  }
}

export async function viewFunction<T>(
  network: Network,
  contractId: string,
  methodName: string,
  args: Record<string, unknown> = {},
): Promise<RpcResult<T>> {
  assertAccountId(contractId);
  if (!/^[a-zA-Z0-9_]+$/.test(methodName)) throw new Error("Invalid view method");
  const response = await fetch(RPC_URLS[network], {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      jsonrpc: "2.0",
      id: "vessel-sdk",
      method: "query",
      params: {
        request_type: "call_function",
        finality: "final",
        account_id: contractId,
        method_name: methodName,
        args_base64: Buffer.from(JSON.stringify(args)).toString("base64"),
      },
    }),
  });
  if (!response.ok) throw new Error(`NEAR RPC returned HTTP ${response.status}`);
  const payload = (await response.json()) as {
    error?: { cause?: { name?: string }; message?: string };
    result?: { result: number[]; block_height: number };
  };
  if (payload.error || !payload.result) {
    throw new Error(payload.error?.cause?.name ?? payload.error?.message ?? "NEAR view call failed");
  }
  const text = Buffer.from(payload.result.result).toString("utf8");
  return { result: JSON.parse(text) as T, blockHeight: payload.result.block_height };
}

export function intentsDepositUrl(verifier: string, recipient: string, assetId: string): string {
  assertAccountId(verifier);
  assertAccountId(recipient);
  if (!assetId || assetId.length > 256) throw new Error("Invalid asset ID");
  const params = new URLSearchParams({ recipient, asset_id: assetId });
  return `https://app.near-intents.org/deposit?${params.toString()}`;
}

export function explorerAccountUrl(network: Network, accountId: string): string {
  assertAccountId(accountId);
  return `https://${network === "mainnet" ? "nearblocks.io" : "testnet.nearblocks.io"}/address/${accountId}`;
}
