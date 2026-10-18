import { afterEach, describe, expect, it, vi } from "vitest";
import { ApiError, api } from "./client";

const respond = (status: number, body?: unknown) =>
  vi.fn().mockImplementation(() =>
    Promise.resolve(
      new Response(body === undefined ? null : JSON.stringify(body), {
        status,
        headers: { "content-type": "application/json" },
      }),
    ),
  );

afterEach(() => vi.unstubAllGlobals());

describe("api client", () => {
  it("requests the usage range as a query parameter", async () => {
    const fetchMock = respond(200, { points: [] });
    vi.stubGlobal("fetch", fetchMock);
    await api.usage("7d");
    expect(fetchMock.mock.calls[0]?.[0]).toBe("/admin-api/usage?range=7d");
  });

  it("raises the platform error message", async () => {
    vi.stubGlobal("fetch", respond(403, { code: 50001, message: "not on the allow-list" }));
    await expect(api.me()).rejects.toMatchObject({ status: 403, message: "not on the allow-list" });
    await expect(api.me()).rejects.toBeInstanceOf(ApiError);
  });

  it("sends the flag body as JSON and encodes the key", async () => {
    const fetchMock = respond(200, {});
    vi.stubGlobal("fetch", fetchMock);
    await api.putFlag("a b", { description: "", enabled: true, rollout_percent: 10 });
    const [url, init] = fetchMock.mock.calls[0] as [string, RequestInit];
    expect(url).toBe("/admin-api/flags/a%20b");
    expect(init.method).toBe("PUT");
    expect(JSON.parse(init.body as string)).toEqual({ description: "", enabled: true, rollout_percent: 10 });
  });

  it("sends the CSRF header on every state-changing request", async () => {
    const fetchMock = respond(204);
    vi.stubGlobal("fetch", fetchMock);
    await api.drainNode("n1");
    await api.deleteFlag("a");
    for (const call of fetchMock.mock.calls) {
      const init = call[1] as RequestInit;
      expect(new Headers(init.headers).get("x-discordy-admin")).toBe("1");
    }
  });

  it("does not send the CSRF header on reads", async () => {
    const fetchMock = respond(200, { points: [] });
    vi.stubGlobal("fetch", fetchMock);
    await api.usage("7d");
    const init = fetchMock.mock.calls[0]?.[1] as RequestInit | undefined;
    expect(new Headers(init?.headers).get("x-discordy-admin")).toBeNull();
  });

  it("treats 204 as an empty success", async () => {
    vi.stubGlobal("fetch", respond(204));
    await expect(api.drainNode("n1")).resolves.toBeUndefined();
  });

  it("omits an empty suspension end", async () => {
    const fetchMock = respond(204);
    vi.stubGlobal("fetch", fetchMock);
    await api.suspendUser("42", "spam", "");
    const init = fetchMock.mock.calls[0]?.[1] as RequestInit;
    expect(JSON.parse(init.body as string)).toEqual({ reason: "spam" });
  });
});
