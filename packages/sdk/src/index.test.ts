import assert from "node:assert/strict";
import test from "node:test";
import { explorerAccountUrl, intentsDepositUrl } from "./index.js";

test("builds safe deposit links", () => {
  const url = intentsDepositUrl("intents.near", "vessel-000001.vessel.near", "nep141:usdc.near");
  assert.match(url, /^https:\/\/app\.near-intents\.org\/deposit\?/);
  assert.match(url, /recipient=vessel-000001\.vessel\.near/);
});

test("rejects malformed accounts", () => {
  assert.throws(() => explorerAccountUrl("mainnet", "not valid"));
});
