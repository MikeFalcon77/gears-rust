const SETTINGS_TOPIC = "demo.settings";
import { describe, expect, it } from "vitest";
import {
  assertOwnEvent,
  Checkpoints,
  ForeignTenantFrame,
  SseDecoder,
  type GearEvent,
} from "./stream-protocol";
const identity = { subject_id: "reader", tenant_id: "dasha" };
const event: GearEvent = {
  id: "event-1",
  type: "settings-changed",
  topic: SETTINGS_TOPIC,
  tenant_id: "dasha",
  source: "settings-service",
  subject: "key",
  occurred_at: "2026-01-01T00:00:00Z",
  partition: 0,
  sequence: 7,
  data: { tenant_id: "dasha" },
};
function storage() {
  const values = new Map<string, string>();
  return {
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => {
      values.set(key, value);
    },
    removeItem: (key: string) => {
      values.delete(key);
    },
  };
}
it("decodes fragmented UTF-8, CRLF and multiple SSE frames", () => {
  const decoder = new SseDecoder(),
    bytes = new TextEncoder().encode(
      ": heartbeat\r\n\r\ndata: " +
        JSON.stringify({
          kind: "event",
          payload: { ...event, subject: "🍦" },
        }) +
        "\r\n\r\ndata: " +
        JSON.stringify({ kind: "heartbeat", at: "now" }) +
        "\n\n",
    );
  const frames = [];
  for (const byte of bytes)
    frames.push(...decoder.push(new Uint8Array([byte])));
  expect(frames).toHaveLength(2);
  expect(frames[0].kind).toBe("event");
  expect(frames[1].kind).toBe("heartbeat");
});
it("fails explicitly instead of hiding a foreign tenant frame", () => {
  expect(() =>
    assertOwnEvent({ ...event, tenant_id: "ilya" }, identity, SETTINGS_TOPIC),
  ).toThrow(ForeignTenantFrame);
  expect(() =>
    assertOwnEvent(
      { ...event, data: { tenant_id: "ilya" } },
      identity,
      SETTINGS_TOPIC,
    ),
  ).toThrow(ForeignTenantFrame);
  expect(() => assertOwnEvent(event, identity, SETTINGS_TOPIC)).not.toThrow();
});
it("rejects oversized frames and unsafe sequence integers", () => {
  expect(() =>
    new SseDecoder().push(
      new TextEncoder().encode("data: " + "x".repeat(1048577)),
    ),
  ).toThrow("Oversized");
  expect(() =>
    assertOwnEvent(
      { ...event, sequence: Number.MAX_SAFE_INTEGER + 1 },
      identity,
      SETTINGS_TOPIC,
    ),
  ).toThrow("sequence");
});
it("persists checkpoints separately for identity, tenant, stream, topic and partition", () => {
  const store = storage(),
    current = new Checkpoints(identity, "tab-one", "settings", store);
  current.event(event);
  current.position(SETTINGS_TOPIC, 1, 19);
  const resumed = new Checkpoints(identity, "tab-one", "settings", store);
  expect(resumed.get(SETTINGS_TOPIC, 0)).toBe(7);
  expect(resumed.get(SETTINGS_TOPIC, 1)).toBe(19);
  expect(resumed.has(event.id)).toBe(true);
  for (const other of [
    new Checkpoints(
      { ...identity, tenant_id: "ilya" },
      "tab-one",
      "settings",
      store,
    ),
    new Checkpoints(
      { ...identity, subject_id: "different" },
      "tab-one",
      "settings",
      store,
    ),
    new Checkpoints(identity, "tab-two", "settings", store),
    new Checkpoints(identity, "tab-one", "another", store),
  ])
    expect(other.get(SETTINGS_TOPIC, 0)).toBeUndefined();
});
it("never regresses a checkpoint and can explicitly reset expired positions", () => {
  const current = new Checkpoints(identity, "tab", "settings", storage());
  current.position(SETTINGS_TOPIC, 0, 30);
  current.event(event);
  expect(current.get(SETTINGS_TOPIC, 0)).toBe(30);
  current.reset();
  expect(current.get(SETTINGS_TOPIC, 0)).toBeUndefined();
  expect(current.has(event.id)).toBe(false);
});
