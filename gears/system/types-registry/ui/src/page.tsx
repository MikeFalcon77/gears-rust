import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { queryKey, type GearRuntime } from "@gears/ui-sdk";
interface Entity {
  id: string;
  gts_id: string;
  is_schema: boolean;
  content: unknown;
  description?: string;
}
export default function TypesRegistry({ runtime }: { runtime: GearRuntime }) {
  const [source, setSource] = useState("gateway"),
    [filter, setFilter] = useState(""),
    [kind, setKind] = useState("all"),
    [selected, setSelected] = useState<string | null>(null);
  const query = useQuery({
    queryKey: queryKey(runtime.identity, "types", source),
    queryFn: ({ signal }) =>
      runtime.api.json<{ entities: Entity[]; count: number }>(
        "/family-labs/v1/registry/" + source,
        { signal },
      ),
  });
  const entities =
      query.data?.entities.filter(
        (e) =>
          (kind === "all" || (kind === "schema") === e.is_schema) &&
          e.gts_id.toLowerCase().includes(filter.toLowerCase()),
      ) || [],
    entity = query.data?.entities.find((e) => e.gts_id === selected);
  return (
    <section className="panel">
      <div className="panel-head">
        <div>
          <h2>Registered contracts & types</h2>
          <p className="muted">
            V1 · Each application owns its registry. Schemas and this lab’s
            instances.
          </p>
        </div>
        <button
          className="secondary"
          disabled={query.isFetching}
          onClick={() => void query.refetch()}
        >
          Refresh registry
        </button>
      </div>
      <div className="toolbar">
        <label>
          Source
          <select
            aria-label="Registry source"
            value={source}
            onChange={(e) => {
              setSource(e.target.value);
              setSelected(null);
            }}
          >
            <option value="gateway">Gateway</option>
            <option value="settings">Settings</option>
          </select>
        </label>
        <label>
          Kind
          <select
            aria-label="Entity kind"
            value={kind}
            onChange={(e) => setKind(e.target.value)}
          >
            <option value="all">All</option>
            <option value="schema">Schemas</option>
            <option value="instance">Instances</option>
          </select>
        </label>
        <input
          aria-label="Filter GTS IDs"
          placeholder="Filter GTS IDs…"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
        />
      </div>
      <p className="technical-note">
        {entities.length} visible · Updated{" "}
        {query.dataUpdatedAt
          ? new Date(query.dataUpdatedAt).toLocaleTimeString()
          : "—"}
      </p>
      {query.error && (
        <p className="error" role="alert">
          {query.error.message}
        </p>
      )}
      {query.isPending ? (
        <div className="empty-state">Reading {source} registry…</div>
      ) : !entities.length ? (
        <div className="empty-state">No matching entities.</div>
      ) : (
        <div className="type-list">
          {entities.map((e) => (
            <button
              key={e.gts_id}
              className={
                "type-row " + (selected === e.gts_id ? "selected" : "")
              }
              onClick={() => setSelected(e.gts_id)}
            >
              <span className="badge">
                {e.is_schema ? "SCHEMA" : "INSTANCE"}
              </span>
              <code>{e.gts_id}</code>
            </button>
          ))}
        </div>
      )}
      {entity && (
        <section className="json-view">
          <div className="panel-head">
            <h3>{entity.is_schema ? "Schema" : "Instance"} JSON</h3>
            <button className="quiet" onClick={() => setSelected(null)}>
              Close
            </button>
          </div>
          <code>{entity.gts_id}</code>
          <pre>{JSON.stringify(entity.content, null, 2)}</pre>
        </section>
      )}
    </section>
  );
}
