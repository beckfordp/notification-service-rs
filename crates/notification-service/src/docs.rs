// SPDX-License-Identifier: MIT

//! The service's API description: what tapir generated from the Scala
//! endpoints (capture 20261111 openapi.yaml) — `/emails` and the two health
//! checks — served with Swagger UI (NOTIF-D1); and the AsyncAPI document of
//! the four topics it consumes, with its page (NOTIF-D2).

use axum::Router;
use serde_json::{Value, json};
use telemetry::docs::asyncapi::{self, Channel};
use telemetry::docs::openapi::{Api, Operation};
use telemetry::docs::swagger;

use crate::domain::USER_REGISTERED;
use crate::emails::SentEmailResponse;
use crate::fulfilment_events::{
    self, DISPATCH_CONFIRMED, DISPATCH_FAILED, DispatchConfirmed, DispatchFailed,
};
use crate::order_events::{
    self, ORDER_CANCELLED, ORDER_CONFIRMED, ORDER_PAYMENT_FAILED, OrderCancelled, OrderConfirmed,
    OrderPaymentFailed,
};
use crate::payment_events::{
    self, PAYMENT_CAPTURED, PAYMENT_FAILED, PaymentCaptured, PaymentFailed,
};
use crate::user_events::{self, UserRegistered};

fn email() -> Value {
    json!({
        "id": "5b1f6c2a-3d4e-4f5a-8b9c-0d1e2f3a4b5c",
        "sourceEventId": "9a8b7c6d-5e4f-4a3b-8c2d-1e0f9a8b7c6d",
        "eventType": "OrderConfirmed",
        "reason": null,
        "recipientType": "customer",
        "recipientKey": "7c9e6679-7425-40de-944b-e07fc1f90ae7",
        "recipientEmail": "ada@example.com",
        "customerId": "7c9e6679-7425-40de-944b-e07fc1f90ae7",
        "orderId": "3f2b8c1e-5d4a-4e7b-9c6f-1a2b3c4d5e6f",
        "subject": "Your order is confirmed",
        "body": "Good news - order 3f2b8c1e-5d4a-4e7b-9c6f-1a2b3c4d5e6f is confirmed.",
        "status": "sent",
        "createdAt": "2026-10-31T10:15:30Z",
        "sentAt": "2026-10-31T10:15:31Z",
        "failedAt": null,
        "error": null,
    })
}

/// The OpenAPI document: the two health checks, then `/emails`.
#[must_use]
pub fn api() -> Api {
    Api::new("Notification Service", "1.0")
        .schema::<SentEmailResponse>()
        .operation(
            Operation::get("/health")
                .tag("health")
                .summary("Liveness check")
                .description("200 while the process is up. Checks no dependencies.")
                .response(200, "", None),
        )
        .operation(
            Operation::get("/health/ready")
                .tag("health")
                .summary("Readiness check")
                .description(
                    "200 when the sent-email audit store (Postgres) is reachable, 503 \
                     otherwise. Kafka consumers are supervised with restart-with-backoff, so \
                     they do not gate readiness.",
                )
                .response(200, "", None)
                .response(503, "Postgres is unreachable.", None),
        )
        .operation(
            Operation::get("/emails")
                .tag("emails")
                .summary("List sent emails")
                .description(
                    "The email audit history, newest first. Both filters are optional and \
                     combined with AND.",
                )
                .query_parameter(
                    "customerId",
                    "Only emails attributed to this customer id.",
                    json!("7c9e6679-7425-40de-944b-e07fc1f90ae7"),
                )
                .query_parameter(
                    "orderId",
                    "Only emails for this order id.",
                    json!("3f2b8c1e-5d4a-4e7b-9c6f-1a2b3c4d5e6f"),
                )
                .response(200, "", Some(("SentEmailResponse[]", json!([email()])))),
        )
}

/// The AsyncAPI document: every event the four consumers act on, received.
#[must_use]
pub fn asyncapi() -> Value {
    asyncapi::document(
        "Notification Service",
        "1.0",
        &[
            Channel::subscribe::<OrderConfirmed>(order_events::TOPIC, ORDER_CONFIRMED),
            Channel::subscribe::<OrderPaymentFailed>(order_events::TOPIC, ORDER_PAYMENT_FAILED),
            Channel::subscribe::<OrderCancelled>(order_events::TOPIC, ORDER_CANCELLED),
            Channel::subscribe::<UserRegistered>(user_events::TOPIC, USER_REGISTERED),
            Channel::subscribe::<DispatchConfirmed>(fulfilment_events::TOPIC, DISPATCH_CONFIRMED),
            Channel::subscribe::<DispatchFailed>(fulfilment_events::TOPIC, DISPATCH_FAILED),
            Channel::subscribe::<PaymentCaptured>(payment_events::TOPIC, PAYMENT_CAPTURED),
            Channel::subscribe::<PaymentFailed>(payment_events::TOPIC, PAYMENT_FAILED),
        ],
    )
}

/// Swagger UI at `/docs/` (`/docs/docs.yaml`), and the AsyncAPI document and page.
pub fn router() -> Router {
    swagger::router(api().to_yaml()).merge(asyncapi::router_with_page(&asyncapi()))
}
