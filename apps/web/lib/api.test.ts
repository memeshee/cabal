import { afterEach, describe, expect, it, vi } from "vitest";
import { isNetworkError, resilientFetch } from "./api";

afterEach(() => vi.restoreAllMocks());

describe("network resilience", () => {
  it("recognises the browser's own connection failures", () => {
    expect(isNetworkError(new TypeError("Failed to fetch"))).toBe(true);
    expect(isNetworkError(new Error("Load failed"))).toBe(true);
    expect(isNetworkError(new Error("The house is unreachable (502)."))).toBe(false);
  });

  it("retries a dropped connection and then succeeds", async () => {
    const ok = new Response("{}", { status: 200 });
    const f = vi.fn().mockRejectedValueOnce(new TypeError("Failed to fetch")).mockRejectedValueOnce(new TypeError("Failed to fetch")).mockResolvedValueOnce(ok);
    vi.stubGlobal("fetch", f);
    const r = await resilientFetch("/x", undefined, 3);
    expect(r).toBe(ok);
    expect(f).toHaveBeenCalledTimes(3);
  });

  it("gives up with a plain message, not the browser's", async () => {
    vi.stubGlobal("fetch", vi.fn().mockRejectedValue(new TypeError("Failed to fetch")));
    await expect(resilientFetch("/x", undefined, 2)).rejects.toThrow(/Can't reach the game server/);
  });

  it("does not retry a refusal from the house", async () => {
    const refusal = new Response('{"ok":false,"error":"all tables are busy"}', { status: 503 });
    const f = vi.fn().mockResolvedValue(refusal);
    vi.stubGlobal("fetch", f);
    const r = await resilientFetch("/x");
    expect(r.status).toBe(503);
    expect(f).toHaveBeenCalledTimes(1);
  });
});
