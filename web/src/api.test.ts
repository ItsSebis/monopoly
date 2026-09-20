import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { deleteRun, getRun, getServerUrl, setServerUrl } from "./api";

beforeEach(() => {
  localStorage.clear();
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("getServerUrl/setServerUrl", () => {
  it("defaults to localhost:3000", () => {
    expect(getServerUrl()).toBe("http://localhost:3000");
  });

  it("persists a custom url", () => {
    setServerUrl("http://example.com");
    expect(getServerUrl()).toBe("http://example.com");
  });
});

describe("request error handling", () => {
  it("resolves with the parsed body on success", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({ ok: true, status: 200, text: async () => '{"id":"run_1"}' }),
    );
    await expect(getRun("run_1")).resolves.toEqual({ id: "run_1" });
  });

  it("throws the server's error message on a non-2xx response", async () => {
    vi.stubGlobal(
      "fetch",
      vi
        .fn()
        .mockResolvedValue({ ok: false, status: 404, text: async () => '{"error":"no run with id run_1"}' }),
    );
    await expect(getRun("run_1")).rejects.toThrow("no run with id run_1");
  });

  it("resolves undefined for a 204 (delete)", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue({ ok: true, status: 204, text: async () => "" }));
    await expect(deleteRun("run_1")).resolves.toBeUndefined();
  });
});
