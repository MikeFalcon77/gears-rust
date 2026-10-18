# Community demo AuthN adapter

AuthN resolver selects this adapter as its single winning plugin. The adapter
calls the Firebase and static plugins through their explicit scoped SDK clients.
JWT-shaped tokens go exclusively to Firebase; failed validation never falls
back. Opaque tokens and client credentials go to the existing static plugin.
The product must configure that delegate in strict static_tokens mode, never
accept_all. Resolver priority is not an authentication chain.
