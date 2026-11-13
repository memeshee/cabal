import { beforeEach, describe, expect, it } from "vitest";
import { loadProgress, saveResult, suggestedTier } from "./progress";

const store: Record<string, string> = {};
beforeEach(() => {
  for (const k of Object.keys(store)) delete store[k];
  (globalThis as unknown as { localStorage: Storage }).localStorage = {
    getItem: (k: string) => store[k] ?? null,
    setItem: (k: string, v: string) => { store[k] = v; },
    removeItem: (k: string) => { delete store[k]; },
    clear: () => {}, key: () => null, length: 0,
  } as Storage;
});

describe("progression", () => {
  it("starts on Rookie and moves up only when a tier is won", () => {
    expect(suggestedTier(loadProgress())).toBe("rookie");
    saveResult("rookie", false);
    expect(suggestedTier(loadProgress())).toBe("rookie");
    saveResult("rookie", true);
    expect(suggestedTier(loadProgress())).toBe("hop");
    saveResult("hop", true);
    expect(suggestedTier(loadProgress())).toBe("ghost");
    saveResult("ghost", true);
    expect(suggestedTier(loadProgress())).toBe("ghost");
  });

  it("remembers results and does not double count", () => {
    saveResult("hop", true);
    saveResult("hop", true);
    expect(loadProgress()).toEqual({ done: ["hop"], won: ["hop"] });
  });

  it("survives a corrupted store", () => {
    store["cabal.progress.v1"] = "{not json";
    expect(loadProgress()).toEqual({ done: [], won: [] });
  });
});
