# Plan — notification-service-rs port

- [x] Clients, recipient lookups, the three topic routes, consumers, config, main `2739c74`
- [x] Docs (OpenAPI, AsyncAPI) `2739c74`
- [x] Dockerfile, deploy values `2739c74`
- [x] Tests; Rust CI 98.25% lines; gluon `bin/verify-port notification-service` all MET
      (contract 76/76, F 34/34, D 2/2) `2739c74`

## Review
Plan compliance yes; NoLookup/NotPorted removed (stale once lookups exist), its tests moved
to local failing resolvers. Low: a lookup's id isn't percent-encoded in the path (http4s
encodes the segment); ids are event UUIDs — recorded as gluon decision A15, not changed.
