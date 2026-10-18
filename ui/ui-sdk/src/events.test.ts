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
function harness(streams: unknown[][]) {
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
