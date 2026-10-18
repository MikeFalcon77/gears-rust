# Family Labs community demo gears

These private, non-published composition modules support the prepared Gearbox
product. They are regular gears with GDL descriptors, not bootstrap SQL scripts.

- **demo-catalogue** restores the demo topic, event schema and subject schema
  through the local Types Registry SDK at each process start. Its EB SDK
  dependency links the base event-schema inventory into both binaries.
- **family-labs-bootstrap** contributes two local-scope Settings declarations
  through SettingsContributionClient. It authenticates the configured bootstrap
  credential after all gears initialize and migrations finish, and signals ready
  only after reconciliation succeeds. It exposes no HTTP bootstrap endpoint.

Repeated boots restore V1 in-memory types and reconcile declaration metadata.
They do not create or reset tenant overrides. Both settings are non-secret,
require normal authenticated access and explicitly disable step-up for this demo.
Real Firebase identities and credentials belong in ignored deployment inputs.
