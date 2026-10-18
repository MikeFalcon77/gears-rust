# Firebase AuthN plugin

Verifies Firebase ID tokens using Google's securetoken JWK endpoint, RS256,
project audience/issuer, expiry and configured UID plus verified-email mappings.
Tenant and scopes come from deployment configuration, never from browser input.
Any number of mappings is supported; no project, accounts or family are built in.

The verifier and adapter were adapted from the owner's
`boring-pills/crates/boring-auth` at the owner's request. The source's signature
and identity regression tests are preserved; project and tenant-specific rules
were replaced with configuration. No dependency on that application remains.
