import { useState, useSyncExternalStore } from "react";
import type { GearRuntime } from "@gears/ui-sdk";
export default function EventsPage({ runtime }: { runtime: GearRuntime }) {
  const state = useSyncExternalStore(
      runtime.events.subscribe,
      runtime.events.getSnapshot,
    ),
    [filter, setFilter] = useState(""),
    [selected, setSelected] = useState<string | null>(null);
  const events = state.events.filter((event) =>
      JSON.stringify(event).toLowerCase().includes(filter.toLowerCase()),
    ),
    event = state.events.find((event) => event.id === selected);
  return (
    <section
      className="panel"
      aria-label="Event Broker inspector"
      data-foreign-tenant-frames={state.foreignTenantFrames}
    >
      <div className="panel-head">
        <div>
          <h2>Events, as they happen</h2>
          <p className="muted">
            A real subscription scoped to your laboratory. Latest 100 events in
            this tab.
          </p>
        </div>
        <span className={"connection " + state.status}>
          <i className="status-dot" />
          {state.status}
        </span>
      </div>
      <div className="toolbar">
        <input
          aria-label="Filter events"
          placeholder="Filter by ID, type, key or tenant…"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
        />
        <button
          className="secondary"
          onClick={() => runtime.events.reconnect()}
        >
          Reconnect
        </button>
      </div>
      {state.error && (
        <p role="alert" className="error">
          {state.error}
        </p>
      )}
      {state.status === "checkpoint_unavailable" && (
        <button onClick={() => runtime.events.startLive()}>Start live</button>
      )}
      <div className="stream-meta">
        <span>
          Tenant <code>{runtime.identity.tenant_id}</code>
        </span>
        <span>Reconnects {state.reconnects}</span>
        <span>
          Heartbeat{" "}
          {state.lastHeartbeat
            ? new Date(state.lastHeartbeat).toLocaleTimeString()
            : "Waiting"}
        </span>
      </div>
      {!events.length ? (
        <div className="empty-state">
          {filter
            ? "No matching events."
            : "Change a setting to see Event Broker deliver it here."}
        </div>
      ) : (
        <div className="event-list">
          {events.map((e) => (
            <button
              className={"event-row " + (selected === e.id ? "selected" : "")}
              key={e.id}
              onClick={() => setSelected(e.id)}
            >
              <span className="event-symbol">↗</span>
              <span>
                <strong>
                  {e.data.key?.split(".").slice(-3, -1).join(".") || e.subject}
                </strong>
                <code>{e.id}</code>
                <small>{e.type}</small>
              </span>
              <time>{new Date(e.occurred_at).toLocaleTimeString()}</time>
            </button>
          ))}
        </div>
      )}
      {event && (
        <section className="json-view">
          <div className="panel-head">
            <h3>Event JSON</h3>
            <button className="quiet" onClick={() => setSelected(null)}>
              Close
            </button>
          </div>
          <pre>{JSON.stringify(event, null, 2)}</pre>
        </section>
      )}
      <p className="technical-note">
        Checkpoints are stored per user, lab, tab, topic and partition. Delivery
        is deduplicated by event ID; the browser does not commit a business
        transaction.
      </p>
    </section>
  );
}
