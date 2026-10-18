import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import { type GearRuntime } from "@gears/ui-sdk";
import { effective, shouldCelebrate } from "./model";
export default function LabWidget({ runtime }: { runtime: GearRuntime }) {
  const feed = useSyncExternalStore(
    runtime.events.subscribe,
    runtime.events.getSnapshot,
  );
  const [value, setValue] = useState<boolean | null>(null),
    [error, setError] = useState(""),
    [eventId, setEventId] = useState(""),
    [celebrations, setCelebrations] = useState(0),
    [burst, setBurst] = useState(false),
    [reads, setReads] = useState(0);
  const mounted = useRef(Date.now());
  const lab = runtime.config.tenants[runtime.identity.tenant_id];
  useEffect(() => {
    let disposed = false,
      current: boolean | null = null;
    const abort = new AbortController(),
      signal = AbortSignal.any([abort.signal, runtime.signal]);
    // Serialize baseline, reconnect and event reads. A slow earlier response
    // cannot overwrite the value established by a later live event.
    let pending: Promise<void> = Promise.resolve();
    const enqueue = (task: () => Promise<void>) => {
      const next = pending.then(task);
      pending = next.catch(() => {});
      return next;
    };
    const failed = (cause: unknown) => {
      if (!signal.aborted) setError((cause as Error).message);
    };
    const read = async () => {
      const result = await effective<boolean>(
        runtime,
        runtime.config.settings.approvalKey,
        signal,
      );
      if (typeof result.value !== "boolean")
        throw Error("Invalid approval value");
      if (disposed) throw new DOMException("Aborted", "AbortError");
      setReads((n) => n + 1);
      setValue(result.value);
      setError("");
      return result.value;
    };
    const baseline = () =>
      enqueue(async () => {
        current = await read();
      }).catch(failed);
    void baseline();
    const remove = runtime.events.onEvent((event) => {
      if (event.data.key !== runtime.config.settings.approvalKey)
        return Promise.resolve();
      return enqueue(async () => {
        const previous = current,
          next = await read();
        current = next;
        setEventId(event.id);
        if (
          shouldCelebrate(previous, next, event.occurred_at, mounted.current)
        ) {
          setCelebrations((n) => n + 1);
          if (!matchMedia("(prefers-reduced-motion: reduce)").matches)
            setBurst(true);
        }
      }).catch((cause) => {
        failed(cause);
        throw cause;
      });
    });
    let lastStatus = "";
    const connected = () => {
      const status = runtime.events.getSnapshot().status;
      if (status === "connected" && lastStatus !== "connected") void baseline();
      lastStatus = status;
    };
    const unsubscribe = runtime.events.subscribe(connected);
    connected();
    return () => {
      disposed = true;
      abort.abort();
      remove();
      unsubscribe();
    };
  }, [runtime]);
  useEffect(() => {
    if (!burst) return;
    const timer = setTimeout(() => setBurst(false), 1600);
    return () => clearTimeout(timer);
  }, [burst]);
  return (
    <section
      className="panel lab-card"
      aria-label="Laboratory approval"
      data-event-id={eventId}
      data-effective-value={String(value)}
      data-celebrations={celebrations}
      data-reads={reads}
    >
      {lab && (
        <>
          <div className="portrait-frame">
            <img
              style={{
                transform: `scale(${lab.portraitZoom || 1})`,
                transformOrigin: lab.portraitPosition || "center",
              }}
              className="portrait"
              src={lab.portrait}
              alt={lab.character + " — " + lab.role}
            />
          </div>
          <p className="eyebrow">{lab.role}</p>
          <h2>{lab.character}'s mission control</h2>
        </>
      )}
      <div className={"approval " + (value ? "approved" : "")} role="status">
        <span className="ice-cream">🍦</span>
        <strong>
          {value === null
            ? "Checking the freezer…"
            : value
              ? "Approved. Deploy the ice cream. 🍦"
              : "Still waiting for approval."}
        </strong>
      </div>
      <p className="muted">
        {feed.status === "connected"
          ? "Live stream connected"
          : feed.status === "reconnecting"
            ? "Reconnecting to Event Broker…"
            : "Event Broker: " + feed.status}
      </p>
      {error && (
        <p role="alert" className="error">
          {error}
        </p>
      )}
      {eventId && (
        <details>
          <summary>Updated by Event Broker</summary>
          <code className="event-id">{eventId}</code>
          <p className="muted">Effective value read again from Settings.</p>
        </details>
      )}
      {burst && (
        <div className="confetti" aria-hidden="true">
          {Array.from({ length: 32 }, (_, i) => (
            <i
              key={i}
              style={{
                left: ((i * 37) % 100) + "%",
                background: ["#0065e3", "#72d4bc", "#f6c85f", "#b799ef"][i % 4],
                animationDelay: (i % 7) * 0.04 + "s",
                transform: "rotate(" + i * 29 + "deg)",
              }}
            />
          ))}
        </div>
      )}
    </section>
  );
}
