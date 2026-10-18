import { useQuery } from "@tanstack/react-query";
import { queryKey, type GearRuntime } from "@gears/ui-sdk";
interface Gear {
  name: string;
  version?: string;
  capabilities: string[];
  dependencies: string[];
  deployment_mode: string;
  instances: Array<{
    instance_id: string;
    version?: string;
    state: string;
    grpc_services: Record<string, string>;
  }>;
  plugins: unknown[];
}
export default function Discovery({ runtime }: { runtime: GearRuntime }) {
  const query = useQuery({
    queryKey: queryKey(runtime.identity, "discovery"),
    queryFn: ({ signal }) =>
      runtime.api.json<Gear[]>("/service-discovery/v1/gears", { signal }),
  });
  return (
    <section className="panel">
      <div className="panel-head">
        <div>
          <h2>The running gear directory</h2>
          <p className="muted">
            Compiled gears and registered out-of-process instances.
          </p>
        </div>
        <button
          className="secondary"
          disabled={query.isFetching}
          onClick={() => void query.refetch()}
        >
          {query.isFetching ? "Refreshing…" : "Refresh directory"}
        </button>
      </div>
      <p className="technical-note">
        Updated{" "}
        {query.dataUpdatedAt
          ? new Date(query.dataUpdatedAt).toLocaleTimeString()
          : "—"}{" "}
        · Service Discovery
      </p>
      {query.error && (
        <p role="alert" className="error">
          {query.error.message}
        </p>
      )}
      {query.isPending ? (
        <div className="empty-state">Reading directory…</div>
      ) : !query.data?.length ? (
        <div className="empty-state">No gears registered.</div>
      ) : (
        <div className="directory-list">
          {query.data.map((gear) => (
            <details key={gear.name}>
              <summary>
                <strong>{gear.name}</strong>
                <span className="badge">
                  {gear.deployment_mode === "out_of_process"
                    ? "OUT OF PROCESS"
                    : "COMPILED IN"}
                </span>
                <span className="muted">
                  {gear.version || gear.instances[0]?.version || "—"}
                </span>
              </summary>
              <p className="muted">
                Capabilities: {gear.capabilities.join(", ") || "None"} ·
                Dependencies: {gear.dependencies.join(", ") || "None"}
              </p>
              {gear.instances.map((instance) => (
                <div key={instance.instance_id} className="instance">
                  <span className="badge">{instance.state}</span>
                  <code>{instance.instance_id}</code>
                  <pre>{JSON.stringify(instance.grpc_services, null, 2)}</pre>
                </div>
              ))}
            </details>
          ))}
        </div>
      )}
    </section>
  );
}
