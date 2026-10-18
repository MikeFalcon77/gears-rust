# Community demo action policy

An explicit demo policy wraps the existing tr-authz scoped SDK. Grants require
a configured token scope, resource and action; property allowlists are exact.
Wildcard bootstrap grants require explicit subject and home-tenant identities.
Unknown tenants and unmatched operations are denied.

A concrete owner_tenant_id forces a single-resource hierarchy decision even
when the PEP has no resource UUID. Global declarations/schema permissions do
not manufacture row ownership. Diagnostic access does not grant setting/event
access in another tenant. Public endpoint scope guards remain in Gateway.
