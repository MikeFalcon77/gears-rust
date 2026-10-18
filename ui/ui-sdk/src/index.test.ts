import { afterEach, describe, expect, it, vi } from "vitest";
import { ApiClient, ApiError, queryKey } from "./index";
afterEach(() => vi.unstubAllGlobals());
describe("authenticated UI transport", () => {
  it("gets the current credential for each request and does not accept an injected Authorization header", async () => {
    let token = "first";
    const fetcher = vi.fn(
      async (_url: string, _options: RequestInit) =>
        new Response("{}", { headers: { "content-type": "application/json" } }),
    );
    vi.stubGlobal("fetch", fetcher);
    const client = new ApiClient(
      "/api",
      async () => token,
      new AbortController().signal,
    );
    await client.json("/one", {
      headers: { Authorization: "Bearer injected" },
    });
    token = "second";
    await client.json("/two");
    expect(
      new Headers(fetcher.mock.calls[0][1]?.headers).get("Authorization"),
    ).toBe("Bearer first");
    expect(
      new Headers(fetcher.mock.calls[1][1]?.headers).get("Authorization"),
    ).toBe("Bearer second");
  });
  it("refuses cross-origin destinations before acquiring credentials", async () => {
    const token = vi.fn(async () => "secret");
    const client = new ApiClient("/api", token, new AbortController().signal);
    for (const route of [
      "//attacker.example/path",
      "https://attacker.example",
      "/../outside",
      "/" + String.fromCharCode(92) + "attacker.example",
    ])
      await expect(client.request(route)).rejects.toThrow();
    expect(token).not.toHaveBeenCalled();
    expect(
      () =>
        new ApiClient(
          "//attacker.example",
          token,
          new AbortController().signal,
        ),
    ).toThrow();
  });
  it("binds in-flight requests to the session lifetime", async () => {
    const lifetime = new AbortController();
    let observed: AbortSignal | undefined;
    vi.stubGlobal(
      "fetch",
      vi.fn(async (_url: string, init: RequestInit) => {
        observed = init.signal as AbortSignal;
        return new Response("{}");
      }),
    );
    await new ApiClient("/api", async () => "token", lifetime.signal).json(
      "/one",
    );
    lifetime.abort();
    expect(observed?.aborted).toBe(true);
  });
  it("preserves canonical HTTP errors", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(
        async () =>
          new Response(
            JSON.stringify({
              status: 403,
              title: "Forbidden",
              detail: "No access",
            }),
            { status: 403 },
          ),
      ),
    );
    try {
      await new ApiClient(
        "/api",
        async () => "token",
        new AbortController().signal,
      ).json("/one");
      throw Error("request should fail");
    } catch (error) {
      expect(error).toBeInstanceOf(ApiError);
      expect((error as ApiError).problem.status).toBe(403);
    }
  });
});
it("partitions all query state by identity and tenant", () => {
  const first = { subject_id: "one", tenant_id: "dasha" };
  expect(queryKey(first, "settings")).not.toEqual(
    queryKey({ ...first, tenant_id: "ilya" }, "settings"),
  );
  expect(queryKey(first, "settings")).not.toEqual(
    queryKey({ ...first, subject_id: "two" }, "settings"),
  );
});
