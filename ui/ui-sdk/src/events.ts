import type { ApiClient, Identity } from "./index";
import {
  assertOwnEvent,
  Checkpoints,
  ForeignTenantFrame,
  SseDecoder,
  type GearEvent,
  type StreamFrame,
  type StreamPosition,
} from "./stream-protocol";
export type ConnectionState =
  | "idle"
  | "connecting"
  | "connected"
  | "reconnecting"
  | "checkpoint_unavailable"
  | "error"
  | "stopped";
export interface FeedSnapshot {
  status: ConnectionState;
  events: readonly GearEvent[];
  foreignTenantFrames: number;
  reconnects: number;
  lastHeartbeat: string | null;
  error: string | null;
}
interface Subscription {
  id: string;
  topology_version: number;
  assigned: Array<{ topic: string; partition: number }>;
}
export interface FeedOptions {
  api: ApiClient;
  identity: Identity;
  signal: AbortSignal;
  tabId: string;
  storage: Pick<Storage, "getItem" | "setItem" | "removeItem">;
  topic: string;
  eventType: string;
  retryMs?: number;
  requiresProcessor?: boolean;
}
class Rejoin extends Error {}
class CheckpointUnavailable extends Error {}
const statusOf = (error: unknown) =>
  (error as { problem?: { status?: number } })?.problem?.status;
function delay(ms: number, signal: AbortSignal) {
  return new Promise<void>((resolve, reject) => {
    if (signal.aborted) {
      reject(signal.reason);
      return;
    }
    const timer = setTimeout(done, ms);
    function done() {
      signal.removeEventListener("abort", abort);
      resolve();
    }
    function abort() {
      clearTimeout(timer);
      reject(signal.reason);
    }
    signal.addEventListener("abort", abort, { once: true });
  });
}
export class EventFeed {
  private snapshot: FeedSnapshot = {
    status: "idle",
    events: [],
    foreignTenantFrames: 0,
    reconnects: 0,
    lastHeartbeat: null,
    error: null,
  };
  private listeners = new Set<() => void>();
  private handlers = new Set<(event: GearEvent) => Promise<void>>();
  private controller: AbortController | undefined;
  private stopped = false;
  private checkpoint: Checkpoints;
  private active: { sub?: string; group: string } | undefined;
  private paused = false;
  private pagehide = (event: PageTransitionEvent) => {
    if (!event.persisted) {
      this.stop();
      return;
    }
    this.paused = true;
    this.controller?.abort();
    void this.cleanup(this.active);
    this.update({ status: "reconnecting" });
  };
  private pageshow = (event: PageTransitionEvent) => {
    if (event.persisted && !this.stopped) {
      this.paused = false;
      this.start();
    }
  };
  constructor(private options: FeedOptions) {
    this.checkpoint = new Checkpoints(
      options.identity,
      options.tabId,
      options.topic,
      options.storage,
    );
    options.signal.addEventListener("abort", () => this.stop(), { once: true });
    if (typeof window !== "undefined") {
      window.addEventListener("pagehide", this.pagehide);
      window.addEventListener("pageshow", this.pageshow);
    }
  }
  getSnapshot = () => this.snapshot;
  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    this.start();
    return () => {
      this.listeners.delete(listener);
      this.releaseIfIdle();
    };
  };
  onEvent = (handler: (event: GearEvent) => Promise<void>) => {
    this.handlers.add(handler);
    this.start();
    return () => {
      this.handlers.delete(handler);
      this.releaseIfIdle();
    };
  };
  private releaseIfIdle() {
    queueMicrotask(() => {
      if (!this.listeners.size && !this.handlers.size) this.controller?.abort();
    });
  }
  private update(value: Partial<FeedSnapshot>) {
    this.snapshot = { ...this.snapshot, ...value };
    for (const listener of this.listeners) listener();
  }
  reconnect() {
    this.update({ status: "reconnecting", error: null });
    this.controller?.abort();
    if (!this.controller) this.start();
  }
  startLive() {
    this.checkpoint.reset();
    this.update({ status: "reconnecting", error: null });
    this.controller?.abort();
    if (!this.controller) this.start();
  }
  stop() {
    this.stopped = true;
    this.controller?.abort();
    this.cleanup(this.active);
    this.update({ status: "stopped" });
    if (typeof window !== "undefined") {
      window.removeEventListener("pagehide", this.pagehide);
      window.removeEventListener("pageshow", this.pageshow);
    }
  }
  private async cleanup(active: typeof this.active) {
    if (!active) return;
    if (active.sub)
      await this.options.api.cleanup(
        "/event-broker/v1/subscriptions/" + encodeURIComponent(active.sub),
      );
    await this.options.api.cleanup(
      "/event-broker/v1/consumer-groups/" + encodeURIComponent(active.group),
    );
  }
  private start() {
    if (
      this.stopped ||
      this.paused ||
      (this.options.requiresProcessor && !this.handlers.size) ||
      this.controller ||
      this.options.signal.aborted ||
      ["error", "checkpoint_unavailable"].includes(this.snapshot.status)
    )
      return;
    const controller = new AbortController();
    this.controller = controller;
    void this.run(controller.signal).finally(() => {
      if (this.controller === controller) {
        this.controller = undefined;
        if (
          !this.stopped &&
          !this.options.signal.aborted &&
          (this.listeners.size || this.handlers.size) &&
          !["error", "checkpoint_unavailable"].includes(this.snapshot.status)
        )
          this.start();
      }
    });
  }
  private async run(signal: AbortSignal) {
    let attempts = 0;
    while (!signal.aborted && !this.options.signal.aborted) {
      let group: string | undefined;
      let sub: string | undefined;
      try {
        this.update({
          status: attempts ? "reconnecting" : "connecting",
          error: null,
        });
        const created = await this.options.api.post<{ id: string }>(
          "/event-broker/v1/consumer-groups",
          {
            client_agent: "family-labs-ui/1.0",
            description: "Browser " + this.options.tabId,
          },
          signal,
        );
        group = created.id;
        this.active = { group };
        const joined = await this.options.api.post<Subscription>(
          "/event-broker/v1/subscriptions",
          {
            consumer_group: group,
            client_agent: "family-labs-ui/1.0",
            interests: [
              {
                topic: this.options.topic,
                tenant_id: this.options.identity.tenant_id,
                max_depth: 0,
                types: [this.options.eventType],
              },
            ],
          },
          signal,
        );
        sub = joined.id;
        this.active = { group, sub };
        if (!joined.assigned.length)
          throw new Rejoin("Waiting for partition assignment");
        const positions: Record<
          string,
          Array<{ partition: number; value: number | "latest" }>
        > = {};
        for (const assignment of joined.assigned) {
          if (
            assignment.topic !== this.options.topic ||
            !Number.isSafeInteger(assignment.partition) ||
            assignment.partition < 0
          )
            throw Error("Unexpected stream assignment");
          (positions[assignment.topic] ??= []).push({
            partition: assignment.partition,
            value:
              this.checkpoint.get(assignment.topic, assignment.partition) ??
              "latest",
          });
        }
        let resolved: {
          positions: Record<
            string,
            Array<{ partition: number; value: number }>
          >;
        };
        try {
          resolved = await this.options.api.post(
            "/event-broker/v1/subscriptions/" +
              encodeURIComponent(sub) +
              ":seek",
            { topology_version: joined.topology_version, positions },
            signal,
          );
        } catch (error) {
          const serialized = JSON.stringify(
            (error as { problem?: unknown })?.problem || {},
          );
          if (
            serialized.includes("below_retention_floor") ||
            serialized.includes("above_high_water_mark")
          )
            throw new CheckpointUnavailable(
              "The saved position is outside the retained event log. Choose Start live to continue.",
            );
          throw error;
        }
        for (const [topic, entries] of Object.entries(resolved.positions))
          for (const entry of entries) {
            if (topic !== this.options.topic)
              throw Error("Unexpected seek topic");
            this.checkpoint.position(topic, entry.partition, entry.value);
          }
        const response = await this.options.api.request(
          "/event-broker/v1/events:sse?subscription_id=" +
            encodeURIComponent(sub),
          { signal, headers: { Accept: "text/event-stream" } },
        );
        if (
          !response.headers
            .get("content-type")
            ?.includes("text/event-stream") ||
          !response.body
        )
          throw Error("The broker did not return an event stream");
        this.update({ status: "connected", error: null });
        const reader = response.body.getReader(),
          decoder = new SseDecoder();
        try {
          while (!signal.aborted) {
            let timer: ReturnType<typeof setTimeout> | undefined;
            const next = await Promise.race([
              reader.read(),
              new Promise<never>((_, reject) => {
                timer = setTimeout(
                  () => reject(new Rejoin("Heartbeat timed out; reconnecting")),
                  30000,
                );
              }),
            ]).finally(() => clearTimeout(timer));
            const { value, done } = next;
            if (done) break;
            for (const frame of decoder.push(value)) {
              if(await this.process(frame)) attempts=0;
            }
          }
        } finally {
          await reader.cancel().catch(() => {});
          reader.releaseLock();
        }
        if (!signal.aborted)
          throw new Rejoin("Stream closed; resuming from checkpoint");
      } catch (error) {
        if (signal.aborted || this.options.signal.aborted) break;
        if (error instanceof ForeignTenantFrame) {
          this.update({
            status: "error",
            foreignTenantFrames: this.snapshot.foreignTenantFrames + 1,
            error: error.message,
          });
          return;
        }
        if (error instanceof CheckpointUnavailable) {
          this.update({
            status: "checkpoint_unavailable",
            error: error.message,
          });
          return;
        }
        const status = statusOf(error);
        if (status === 403 || status === 401) {
          this.update({
            status: "error",
            error: "This session cannot read the event stream. Sign in again.",
          });
          return;
        }
        attempts++;
        this.update({
          status: "reconnecting",
          reconnects: this.snapshot.reconnects + 1,
          error:
            error instanceof Error
              ? error.message
              : "The event stream is unavailable.",
        });
      } finally {
        await this.cleanup(group ? { group, sub } : undefined);
        this.active = undefined;
      }
      try {
        await delay(
          Math.min(
            (this.options.retryMs ?? 500) * 2 ** Math.min(attempts - 1, 4),
            5000,
          ),
          signal,
        );
      } catch {
        break;
      }
    }
  }
  private positions(positions: StreamPosition[]) {
    for (const position of positions) {
      if (position.topic !== this.options.topic)
        throw Error("Unexpected control topic");
      this.checkpoint.position(
        position.topic,
        position.partition,
        Math.max(position.offset, position.last_examined),
      );
    }
  }
  private async process(frame: StreamFrame) {
    switch (frame.kind) {
      case "event": {
        const event = frame.payload;
        assertOwnEvent(event, this.options.identity, this.options.topic);
        if (event.type !== this.options.eventType)
          throw Error("Unexpected event type");
        if (this.checkpoint.has(event.id)) {
          this.checkpoint.position(
            event.topic,
            event.partition,
            event.sequence,
          );
          return;
        }
        try {
          for (const handler of this.handlers) await handler(event);
        } catch (cause) {
          // A processor's 401/403 is not an EB login failure. Preserve the
          // checkpoint and retry processing after rejoining the stream.
          throw new Rejoin(
            "Event processing failed: " +
              (cause instanceof Error
                ? cause.message
                : "retrying the effective read"),
          );
        }
        if (this.options.signal.aborted) return;
        this.checkpoint.event(event);
        this.update({ events: [event, ...this.snapshot.events].slice(0, 100) });
        return true;
      }
      case "heartbeat":
        this.update({ lastHeartbeat: frame.at });
        return true;
      case "topology":
        this.positions(frame.assigned);
        return;
      case "control":
        this.positions(frame.positions);
        if (frame.code === "terminal")
          throw new Rejoin(frame.reason || "Broker closed the stream");
        if (frame.code !== "progress")
          throw Error("Unknown stream control code");
        return;
    }
  }
}
