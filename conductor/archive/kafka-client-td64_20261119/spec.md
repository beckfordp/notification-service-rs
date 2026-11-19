# Spec — adopt kafka-client's TD-64 changes

## Overview
gluon-rs track `kafka-client-td64_20261119` (gluon TD-64.1–64.5) changed the Kafka crates'
API: `Routes` declares its own `type Error`, and `ConsumerSettings` takes `GroupId` and
`Topic`. This service adopts them, with no behaviour change.

## Functional Requirements
1. `UserEvents`' route returns a typed `UserEventsError` (payload decode, send), not
   `String`; each reads as the message it replaced.
2. The consumer's group and topic are parsed into `GroupId` and `Topic`; an invalid one
   fails the consumers at startup (`KafkaError::ClientCreation`), never panics.

## Acceptance Criteria
- [ ] `scripts/ci.sh` green; `bin/verify-port notification-service` 38/38 (cluster not run
      on macOS).
- [ ] No `Result<_, String>` in the route.

## Out of Scope
`lifecycle`'s `Result<(), String>` futures (a separate TD-66 item), behaviour changes.
