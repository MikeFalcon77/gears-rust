import { useQuery } from "@tanstack/react-query";
import {
  effective,
  queryKey,
  ICE_CREAM_KEY,
  LAB_MESSAGE_KEY,
  type GearRuntime,
} from "@gears/ui-sdk";
export default function SettingsPage({ runtime }: { runtime: GearRuntime }) {
  const approval = useQuery({
    queryKey: queryKey(runtime.identity, "effective", ICE_CREAM_KEY),
    queryFn: ({ signal }) => effective<boolean>(runtime, ICE_CREAM_KEY, signal),
  });
  const message = useQuery({
    queryKey: queryKey(runtime.identity, "effective", LAB_MESSAGE_KEY),
    queryFn: ({ signal }) =>
      effective<string>(runtime, LAB_MESSAGE_KEY, signal),
  });
  if (approval.isPending || message.isPending)
    return (
      <div className="empty-state" role="status">
        Reading effective settings…
      </div>
    );
  if (approval.error || message.error)
    return (
      <p className="error" role="alert">
        {(approval.error || message.error)?.message}
      </p>
    );
  return (
    <section className="panel">
      <div className="panel-head">
        <div>
          <h2>Your laboratory settings</h2>
          <p className="muted" style={{ marginTop: 8 }}>
            Resolved by the Settings worker through the generated remote
            contract.
          </p>
        </div>
        <span className="badge">LIVE BACKEND</span>
      </div>
      <div className="setting-row">
        <div>
          <h3>Ice cream approval</h3>
          <p className="muted">
            {approval.data?.value
              ? "Approved. Deploy the ice cream. 🍦"
              : "Still waiting for approval."}
          </p>
        </div>
        <span className="badge">
          {approval.data?.source.replaceAll("_", " ")}
        </span>
      </div>
      <div className="setting-row">
        <div>
          <h3>Lab message</h3>
          <p className="muted">{message.data?.value}</p>
        </div>
        <span className="badge">
          {message.data?.source.replaceAll("_", " ")}
        </span>
      </div>
    </section>
  );
}
