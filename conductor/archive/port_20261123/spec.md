# Spec — notification-service-rs: the remaining port (gluon all-Rust Stage A)

## Overview
The service consumes user-events only. Port the rest of the Scala service so the
harness's NOTIF-F16–F34 and D1–D2 pass and the image deploys to gluon-local.

## Functional requirements
1. Recipient lookups (Scala `RecipientResolver`, `UserClient`, `OrderClient`): user-service
   `GET /users/{id}` and order-service `GET /orders/{id}` through gluon-rs
   `ResilientClient` (3 retries from 100 ms, breaker 5 failures / 30 s each); 400/404 are
   "not found"; the order's guest email as fallback; `unknown_order:…`,
   `unknown_customer:…`, `no_customer_identity` unresolved.
2. Consumers for order-events (OrderConfirmed, OrderPaymentFailed, OrderCancelled),
   payment-events (PaymentCaptured, PaymentFailed), fulfilment-events (DispatchConfirmed,
   DispatchFailed), the Scala copy verbatim, through the audited send; others skipped,
   undecodable dead-lettered.
3. Config `USER_SERVICE_URL` (default http://localhost:8084), `ORDER_SERVICE_URL`
   (http://localhost:8083).
4. Docs: Swagger UI + OpenAPI (`/emails`, health), AsyncAPI receive channels and page.
5. `Dockerfile` (port 8080) and `deploy/notification-service-rs.values.yaml`.

## Acceptance criteria
Rust CI green (≥90% lines); gluon `bin/verify-port notification-service` all MET.

## Out of scope
Wire rows W15–W23 (bodies byte for byte are met anyway; metrics, traces, logs).
