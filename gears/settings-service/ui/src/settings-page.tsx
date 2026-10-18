import { useEffect, useState } from "react";
import { ApiError, type GearRuntime } from "@gears/ui-sdk";
import { parseValue } from "./model";
type Field = GearRuntime["config"]["settings"]["fields"][number];
interface ValueState {
  value: unknown;
  etag: string;
  source: string;
  source_scope?: string;
  inheritance_trail?: unknown[];
}
function Editor({ runtime, field }: { runtime: GearRuntime; field: Field }) {
  const [state, setState] = useState<ValueState | null>(null),
    [draft, setDraft] = useState(""),
    [busy, setBusy] = useState(false),
    [error, setError] = useState(""),
    [conflict, setConflict] = useState(false),
    [history, setHistory] = useState<unknown | null>(null);
  const base = "/settings-service/v1/settings/" + encodeURIComponent(field.key);
  const load = async () => {
    setBusy(true);
    setError("");
    try {
      const response = await runtime.api.request(base);
      const value = (await response.json()) as ValueState;
      value.etag = response.headers.get("etag") || value.etag;
      if (!value.etag) throw Error("The server did not return a version tag.");
      setState(value);
      setDraft(String(value.value));
      setConflict(false);
    } catch (cause) {
      setError((cause as Error).message);
    } finally {
      setBusy(false);
    }
  };
  useEffect(() => {
    void load();
  }, [runtime, field.key]);
  const save = async () => {
    if (!state) return;
    setBusy(true);
    setError("");
    try {
      const value = parseValue(field.kind, draft);
      await runtime.api.json(base + "/value", {
        method: "PUT",
        headers: {
          "If-Match": state.etag,
          "X-Request-Id": crypto.randomUUID(),
        },
        body: JSON.stringify({ value }),
      });
      runtime.notify(field.label + " saved.");
      await load();
    } catch (cause) {
      setConflict(cause instanceof ApiError && cause.problem.status === 412);
      setError(
        cause instanceof ApiError && cause.problem.status === 412
          ? "Someone changed this setting. Your draft is preserved. Reload the current value before editing again."
          : (cause as Error).message,
      );
    } finally {
      setBusy(false);
    }
  };
  return (
    <article className="setting-editor">
      <div className="panel-head">
        <div>
          <h3>{field.label}</h3>
          <p className="muted">
            {field.kind} · {state?.source.replaceAll("_", " ") || "Loading…"}
          </p>
        </div>
        <span className="badge">
          {state?.source === "own_override"
            ? "THIS LAB"
            : "DEFAULT / INHERITED"}
        </span>
      </div>
      <label className="field-label">
        {field.label}
        {field.kind === "boolean" ? (
          <select
            aria-label={field.label}
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            disabled={busy || !state}
          >
            <option value="false">Not yet</option>
            <option value="true">Approved</option>
          </select>
        ) : (
          <input
            aria-label={field.label}
            type={field.kind === "number" ? "number" : "text"}
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            disabled={busy || !state}
            maxLength={field.kind === "string" ? 1000 : undefined}
          />
        )}
      </label>
      <div className="button-row">
        <button
          onClick={() => void save()}
          disabled={busy || !state || conflict}
        >
          {busy ? "Working…" : "Save " + field.label.toLowerCase()}
        </button>
        <button
          className="secondary"
          onClick={() => void load()}
          disabled={busy}
        >
          Reload current value
        </button>
        <button
          className="quiet"
          disabled={busy}
          onClick={() => {
            setError("");
            void runtime.api
              .json(base + "/history?limit=10")
              .then(setHistory)
              .catch((cause) => setError(cause.message));
          }}
        >
          History
        </button>
      </div>
      {error && (
        <p role="alert" className="error">
          {error}
        </p>
      )}
      {state && (
        <details>
          <summary>Resolution & version</summary>
          <code className="event-id">ETag: {state.etag}</code>
          <pre>
            {JSON.stringify(
              {
                source: state.source,
                sourceScope: state.source_scope,
                trail: state.inheritance_trail,
              },
              null,
              2,
            )}
          </pre>
        </details>
      )}
      {history !== null && (
        <details open>
          <summary>Recent changes</summary>
          <pre>{JSON.stringify(history, null, 2)}</pre>
        </details>
      )}
    </article>
  );
}
export default function SettingsPage({ runtime }: { runtime: GearRuntime }) {
  const [echo, setEcho] = useState<unknown>(null),
    [error, setError] = useState(""),
    [busy, setBusy] = useState(false);
  return (
    <>
      <section className="panel">
        <div className="panel-head">
          <div>
            <h2>Your laboratory settings</h2>
            <p className="muted">
              Changes are versioned, validated and published through Event
              Broker.
            </p>
          </div>
          <span className="badge">LIVE BACKEND</span>
        </div>
        {runtime.config.settings.fields.map((field) => (
          <Editor key={field.key} runtime={runtime} field={field} />
        ))}
      </section>
      <section className="panel section-gap">
        <div className="panel-head">
          <div>
            <h2>A round trip through OAGW</h2>
            <p className="muted">
              Call the echo service using this laboratory’s upstream.
            </p>
          </div>
        </div>
        <button
          disabled={busy}
          onClick={() => {
            setBusy(true);
            setError("");
            void runtime.api
              .json(runtime.config.settings.echoPath, {
                headers: { "X-Request-Id": crypto.randomUUID() },
              })
              .then(setEcho)
              .catch((cause) => setError(cause.message))
              .finally(() => setBusy(false));
          }}
        >
          {busy ? "Calling…" : "Send echo request"}
        </button>
        {error && (
          <p className="error" role="alert">
            {error}
          </p>
        )}
        {echo !== null && <pre>{JSON.stringify(echo, null, 2)}</pre>}
      </section>
    </>
  );
}
