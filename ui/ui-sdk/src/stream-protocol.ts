import type { Identity } from "./index";
export interface GearEvent {
  id: string;
  type: string;
  topic: string;
  tenant_id: string;
  source: string;
  subject: string;
  occurred_at: string;
  partition: number;
  sequence: number;
  data: {
    key?: string;
    change_set_id?: string;
    request_id?: string;
    tenant_id?: string;
    [key: string]: unknown;
  };
}
export interface StreamPosition {
  topic: string;
  partition: number;
  offset: number;
  last_examined: number;
}
export type StreamFrame =
  | { kind: "event"; payload: GearEvent }
  | { kind: "heartbeat"; at: string }
  | { kind: "topology"; topology_version: number; assigned: StreamPosition[] }
  | {
      kind: "control";
      code: "progress" | "terminal";
      positions: StreamPosition[];
      reason?: "rebalanced" | "lose_all" | "teardown";
    };
export class ForeignTenantFrame extends Error {
  constructor() {
    super("The stream received a foreign tenant frame. Connection stopped.");
  }
}
export function assertOwnEvent(
  event: GearEvent,
  identity: Identity,
  topic: string,
) {
  if (
    !event ||
    event.tenant_id !== identity.tenant_id ||
    (event.data?.tenant_id && event.data.tenant_id !== identity.tenant_id)
  )
    throw new ForeignTenantFrame();
  if (
    event.topic !== topic ||
    !event.id ||
    !Number.isSafeInteger(event.partition) ||
    event.partition < 0 ||
    !Number.isSafeInteger(event.sequence) ||
    event.sequence < 0
  )
    throw Error("Invalid event identity or sequence");
}
export class SseDecoder {
  private buffer = "";
  private decoder = new TextDecoder();
  push(bytes: Uint8Array): StreamFrame[] {
    this.buffer += this.decoder.decode(bytes, { stream: true });
    const result: StreamFrame[] = [];
    let boundary: RegExpExecArray | null;
    while ((boundary = /\r?\n\r?\n/.exec(this.buffer))) {
      const frame = this.buffer.slice(0, boundary.index);
      this.buffer = this.buffer.slice(boundary.index + boundary[0].length);
      if (frame.length > 1048576) throw Error("Oversized stream frame");
      const data = frame
        .split(/\r?\n/)
        .filter((line) => line.startsWith("data:"))
        .map((line) => line.slice(5).trimStart())
        .join("\n");
      if (data) {
        const decoded = JSON.parse(data);
        if (
          !["event", "heartbeat", "topology", "control"].includes(decoded.kind)
        )
          throw Error("Unknown stream frame");
        result.push(decoded as StreamFrame);
      }
    }
    if (this.buffer.length > 1048576) throw Error("Oversized stream frame");
    return result;
  }
}
export interface Checkpoint {
  version: 1;
  positions: Record<string, Record<string, number>>;
  seen: string[];
}
export class Checkpoints {
  private current: Checkpoint = { version: 1, positions: {}, seen: [] };
  readonly key: string;
  constructor(
    identity: Identity,
    tabId: string,
    stream: string,
    private storage: Pick<Storage, "getItem" | "setItem" | "removeItem">,
  ) {
    this.key = [
      "gears.ui.checkpoint",
      identity.subject_id,
      identity.tenant_id,
      tabId,
      stream,
    ]
      .map(encodeURIComponent)
      .join(":");
    try {
      const saved = JSON.parse(storage.getItem(this.key) || "null");
      if (saved?.version === 1 && saved.positions && Array.isArray(saved.seen))
        this.current = saved;
    } catch {
      /* Storage may be unavailable or contain obsolete data. */
    }
  }
  get(topic: string, partition: number) {
    const value = this.current.positions[topic]?.[partition];
    return Number.isSafeInteger(value) && value >= 0 ? value : undefined;
  }
  has(id: string) {
    return this.current.seen.includes(id);
  }
  event(event: GearEvent) {
    this.position(event.topic, event.partition, event.sequence);
    if (!this.has(event.id)) this.current.seen.push(event.id);
    this.current.seen = this.current.seen.slice(-1000);
    this.save();
  }
  position(topic: string, partition: number, offset: number) {
    if (
      !Number.isSafeInteger(partition) ||
      partition < 0 ||
      !Number.isSafeInteger(offset) ||
      offset < 0
    )
      throw Error("Invalid stream position");
    const positions = this.current.positions[topic] || {};
    positions[partition] = Math.max(positions[partition] ?? 0, offset);
    this.current.positions[topic] = positions;
    this.save();
  }
  reset() {
    this.current = { version: 1, positions: {}, seen: [] };
    try {
      this.storage.removeItem(this.key);
    } catch {}
  }
  private save() {
    try {
      this.storage.setItem(this.key, JSON.stringify(this.current));
    } catch {
      /* In-memory checkpoints still protect reconnects. */
    }
  }
}
// window.name is per browsing context and survives reload. sessionStorage is
// copied into opener-created tabs and therefore cannot supply a unique tab ID.
export function browserTabId() {
  const prefix = "gears.ui.tab:";
  if (
    !window.name.startsWith(prefix) ||
    !/^gears\.ui\.tab:[0-9a-f-]{36}$/.test(window.name)
  )
    window.name = prefix + crypto.randomUUID();
  return window.name.slice(prefix.length);
}
