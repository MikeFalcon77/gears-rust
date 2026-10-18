import { expect, it, vi } from "vitest";
import { EventFeed } from "./events";
import type { ApiClient } from "./index";
import type { GearEvent } from "./stream-protocol";
const identity = { subject_id: "one", tenant_id: "dasha" },
  topic = "demo.settings";
const event = (id: string, sequence: number): GearEvent => ({
  id,
  sequence,
  partition: 0,
  topic,
  tenant_id: "dasha",
  type: "changed",
  source: "settings",
  subject: "key",
  occurred_at: new Date().toISOString(),
  data: { tenant_id: "dasha" },
});
function harness(streams: unknown[][], requiresProcessor = false) {
  const lifetime = new AbortController(),
    saved = new Map<string, string>();
  let joins = 0,
    connections = 0;
  const seeks: unknown[] = [];
  const api = {
    post: vi.fn(async (route: string, body: any) => {
      if (route.endsWith("/consumer-groups")) return { id: "group-" + ++joins };
      if (route.endsWith("/subscriptions"))
        return {
          id: "sub-" + joins,
          topology_version: 1,
          assigned: [{ topic, partition: 0 }],
        };
      if (route.endsWith(":seek")) {
        seeks.push(body.positions[topic][0].value);
        return {
          positions: {
            [topic]: [
              {
                partition: 0,
                value:
                  typeof body.positions[topic][0].value === "number"
                    ? body.positions[topic][0].value
                    : 0,
              },
            ],
          },
        };
      }
      throw Error("Unexpected route " + route);
    }),
    request: vi.fn(async (_route: string, options: RequestInit) => {
      const frames = streams[connections++] || [];
      const close = connections < streams.length;
      const body = new ReadableStream<Uint8Array>({
        start(controller) {
          for (const frame of frames)
            controller.enqueue(
              new TextEncoder().encode(
                "data: " + JSON.stringify(frame) + "\n\n",
              ),
            );
          if (close) controller.close();
          else
            options.signal?.addEventListener(
              "abort",
              () => controller.error(new DOMException("Aborted", "AbortError")),
              { once: true },
            );
        },
      });
      return new Response(body, {
        headers: { "content-type": "text/event-stream" },
      });
    }),
    cleanup: vi.fn(async () => {}),
  };
  const feed = new EventFeed({
    api: api as unknown as ApiClient,
    identity,
    signal: lifetime.signal,
    tabId: "tab",
    storage: {
      getItem: (key) => saved.get(key) ?? null,
      setItem: (key, value) => {
        saved.set(key, value);
      },
      removeItem: (key) => {
        saved.delete(key);
      },
    },
    topic,
    eventType: "changed",
    retryMs: 1,
    requiresProcessor,
  });
  return { api, feed, seeks, lifetime, saved };
}
it("rejoins after EOF, seeks the processed position and deduplicates a replay", async () => {
  const one = event("one", 7),
    two = event("two", 8),
    h = harness([
      [
        { kind: "event", payload: one },
        {
          kind: "control",
          code: "progress",
          positions: [{ topic, partition: 0, offset: 7, last_examined: 7 }],
        },
      ],
      [
        { kind: "event", payload: one },
        { kind: "event", payload: two },
      ],
    ]);
  const handler = vi.fn(async () => {}),
    remove = h.feed.onEvent(handler),
    unsubscribe = h.feed.subscribe(() => {});
  try {
    await vi.waitFor(() => expect(h.feed.getSnapshot().events).toHaveLength(2));
    expect(handler).toHaveBeenCalledTimes(2);
    expect(h.seeks).toEqual(["latest", 7]);
    expect(h.feed.getSnapshot().events.map((e) => e.id)).toEqual([
      "two",
      "one",
    ]);
    expect(h.feed.getSnapshot().reconnects).toBe(1);
  } finally {
    h.lifetime.abort();
    remove();
    unsubscribe();
  }
});
it("does not advance a checkpoint when the effective-value read fails", async () => {
  const one = event("one", 7),
    h = harness([
      [{ kind: "event", payload: one }],
      [{ kind: "event", payload: one }],
    ]);
  let attempts = 0;
  const remove = h.feed.onEvent(async () => {
      if (++attempts === 1) throw Error("Settings temporarily unavailable");
    }),
    unsubscribe = h.feed.subscribe(() => {});
  try {
    await vi.waitFor(() => expect(h.feed.getSnapshot().events).toHaveLength(1));
    expect(h.seeks).toEqual(["latest", 0]);
    expect(attempts).toBe(2);
  } finally {
    h.lifetime.abort();
    remove();
    unsubscribe();
  }
});
it("stops explicitly on a foreign tenant and cleans up the browser subscription", async () => {
  const h = harness([
      [
        {
          kind: "event",
          payload: { ...event("foreign", 7), tenant_id: "ilya" },
        },
      ],
    ]),
    handler = vi.fn(async () => {}),
    remove = h.feed.onEvent(handler),
    unsubscribe = h.feed.subscribe(() => {});
  try {
    await vi.waitFor(() => expect(h.feed.getSnapshot().status).toBe("error"));
    expect(h.feed.getSnapshot().foreignTenantFrames).toBe(1);
    expect(handler).not.toHaveBeenCalled();
    expect(h.feed.getSnapshot().events).toHaveLength(0);
    expect(h.api.cleanup).toHaveBeenCalledWith(
      "/event-broker/v1/subscriptions/sub-1",
    );
  } finally {
    h.lifetime.abort();
    remove();
    unsubscribe();
  }
});

it.each([401, 403])(
  "retries a processor HTTP %i without falsely reporting an EB login failure",
  async (status) => {
    const one = event("read-retry", 9),
      h = harness([
        [{ kind: "event", payload: one }],
        [{ kind: "event", payload: one }],
      ]);
    let attempts = 0;
    const remove = h.feed.onEvent(async () => {
        if (++attempts === 1) throw { problem: { status } };
      }),
      unsubscribe = h.feed.subscribe(() => {});
    try {
      await vi.waitFor(() =>
        expect(h.feed.getSnapshot().events).toHaveLength(1),
      );
      expect(h.feed.getSnapshot().status).toBe("connected");
      expect(h.seeks).toEqual(["latest", 0]);
      expect(attempts).toBe(2);
    } finally {
      h.lifetime.abort();
      remove();
      unsubscribe();
    }
  },
);

it("waits for the required processor before opening a stream", async () => {
  const h = harness(
      [[{ kind: "event", payload: event("after-ready", 1) }]],
      true,
    ),
    unsubscribe = h.feed.subscribe(() => {});
  await Promise.resolve();
  expect(h.api.post).not.toHaveBeenCalled();
  const handler = vi.fn(async () => {}),
    remove = h.feed.onEvent(handler);
  try {
    await vi.waitFor(() => expect(handler).toHaveBeenCalledOnce());
    expect(h.feed.getSnapshot().events).toHaveLength(1);
  } finally {
    h.lifetime.abort();
    remove();
    unsubscribe();
  }
});

it("uses topology/terminal positions when rejoining a reassigned stream", async () => {
  const position = { topic, partition: 0, offset: 8, last_examined: 8 },
    h = harness([
      [
        {
          kind: "topology",
          topology_version: 2,
          assigned: [{ ...position, offset: 7, last_examined: 7 }],
        },
        { kind: "heartbeat", at: "2026-10-18T10:00:00Z" },
        {
          kind: "control",
          code: "terminal",
          reason: "rebalanced",
          positions: [position],
        },
      ],
      [],
    ]);
  const unsubscribe = h.feed.subscribe(() => {});
  try {
    await vi.waitFor(() => expect(h.api.request).toHaveBeenCalledTimes(2));
    expect(h.seeks).toEqual(["latest", 8]);
    expect(h.feed.getSnapshot().lastHeartbeat).toBe("2026-10-18T10:00:00Z");
  } finally {
    h.lifetime.abort();
    unsubscribe();
  }
});
it("recovers when a persisted page returns from the back-forward cache", async () => {
  const windowEvents = new EventTarget();
  vi.stubGlobal("window", windowEvents);
  const h = harness([[]]),
    unsubscribe = h.feed.subscribe(() => {});
  try {
    await vi.waitFor(() =>
      expect(h.feed.getSnapshot().status).toBe("connected"),
    );
    const hide = new Event("pagehide");
    Object.defineProperty(hide, "persisted", { value: true });
    windowEvents.dispatchEvent(hide);
    await vi.waitFor(() => expect(h.api.cleanup).toHaveBeenCalled());
    const show = new Event("pageshow");
    Object.defineProperty(show, "persisted", { value: true });
    windowEvents.dispatchEvent(show);
    await vi.waitFor(() => expect(h.api.request).toHaveBeenCalledTimes(2));
    expect(h.feed.getSnapshot().status).toBe("connected");
  } finally {
    h.lifetime.abort();
    unsubscribe();
    vi.unstubAllGlobals();
  }
});
it("reconnects after the broker heartbeat deadline", async () => {
  vi.useFakeTimers();
  const h = harness([[]]),
    unsubscribe = h.feed.subscribe(() => {});
  try {
    await vi.waitFor(() =>
      expect(h.feed.getSnapshot().status).toBe("connected"),
    );
    await vi.advanceTimersByTimeAsync(30002);
    expect(h.api.request).toHaveBeenCalledTimes(2);
    expect(h.feed.getSnapshot().reconnects).toBe(1);
  } finally {
    h.lifetime.abort();
    unsubscribe();
    vi.useRealTimers();
  }
});
it("shows an expired checkpoint and resumes only after explicit Start live", async () => {
  const h = harness([[]]),
    original = h.api.post.getMockImplementation()!;
  let denied = false;
  h.api.post.mockImplementation(async (route, body) => {
    if (route.endsWith(":seek") && !denied) {
      denied = true;
      throw {
        problem: {
          status: 400,
          context: {
            field_violations: [
              { field: "positions", reason: "below_retention_floor" },
            ],
          },
        },
      };
    }
    return original(route, body);
  });
  const unsubscribe = h.feed.subscribe(() => {});
  try {
    await vi.waitFor(() =>
      expect(h.feed.getSnapshot().status).toBe("checkpoint_unavailable"),
    );
    expect(h.api.request).not.toHaveBeenCalled();
    h.feed.startLive();
    await vi.waitFor(() =>
      expect(h.feed.getSnapshot().status).toBe("connected"),
    );
    expect(h.seeks).toEqual(["latest"]);
  } finally {
    h.lifetime.abort();
    unsubscribe();
  }
});
