// SPDX-License-Identifier: MIT

//! user-service's and order-service's lookups (Scala `UserClient`,
//! `OrderClient`), each through purerest's resilience (gluon-rs `telemetry`'s
//! `ResilientClient`: retry outside its own circuit breaker,
//! `application.conf`'s 3 retries from 100 ms and 5 failures for 30 s). A 400
//! or 404 is "not found"; anything else (another status after the retries, an
//! open breaker, a transport failure, a body it can't read) is a
//! [`CallError`], which the audited send records and raises for the DLQ guard.

use std::time::Duration;

use reqwest::StatusCode;
use serde::Deserialize;
use telemetry::client::{ClientError, ResilientClient, TracedClient};
use telemetry::metrics::Metrics;
use telemetry::resilience::{CircuitBreakerConfig, RetryConfig};
use telemetry::tracing::SdkTracer;

use crate::domain::{CustomerId, EmailAddress, OrderContact, OrderId};
use crate::email::LookupError;
use crate::recipients::{CustomerEmails, OrderContacts};

/// Each request's timeout.
pub const TIMEOUT: Duration = Duration::from_secs(10);

/// `application.conf`'s `user-client.resilience.retry` (`order-client`'s too).
pub const RETRY: RetryConfig = RetryConfig {
    max_retries: 3,
    base_delay: Duration::from_millis(100),
};

/// `application.conf`'s `user-client.resilience.circuit-breaker` (`order-client`'s
/// too).
pub const BREAKER: CircuitBreakerConfig = CircuitBreakerConfig {
    failure_threshold: 5,
    reset_timeout: Duration::from_secs(30),
};

/// A lookup that gave no answer.
#[derive(Debug, thiserror::Error)]
pub enum CallError {
    /// No response: an open breaker (`Circuit breaker is open`) or a transport
    /// failure.
    #[error(transparent)]
    Client(#[from] ClientError),
    /// A status other than 200, 400 or 404, as the Scala message words it.
    #[error("Unexpected response from {service} for {id}: {status}")]
    Unexpected {
        /// `user-service` or `order-service`.
        service: &'static str,
        /// The id looked up.
        id: String,
        /// The status.
        status: StatusCode,
    },
    /// A 200 whose body isn't what the Scala decoder needs.
    #[error("{0}")]
    Body(String),
}

impl From<CallError> for LookupError {
    fn from(error: CallError) -> Self {
        Self::Failed(error.to_string())
    }
}

/// A client behind its own breaker (one per callee).
///
/// # Errors
/// When the HTTP client can't be built.
pub fn resilient(tracer: SdkTracer, metrics: &Metrics) -> Result<ResilientClient, reqwest::Error> {
    Ok(ResilientClient::new(
        TracedClient::new(TIMEOUT, tracer, metrics)?,
        RETRY,
        BREAKER,
        metrics,
    ))
}

/// `GET {base_url}/{collection}/{id}`: the 200's body, `None` for 400 or 404.
async fn lookup<T: for<'de> Deserialize<'de>>(
    client: &ResilientClient,
    base_url: &str,
    service: &'static str,
    collection: &str,
    id: &str,
) -> Result<Option<T>, CallError> {
    let response = client.get(&format!("{base_url}/{collection}/{id}")).await?;
    match response.status() {
        StatusCode::OK => response
            .json()
            .await
            .map(Some)
            .map_err(|error| CallError::Body(error.to_string())),
        StatusCode::BAD_REQUEST | StatusCode::NOT_FOUND => Ok(None),
        status => Err(CallError::Unexpected {
            service,
            id: id.to_owned(),
            status,
        }),
    }
}

fn base(url: &str) -> String {
    url.trim_end_matches('/').to_owned()
}

// The field the Scala decoder needs; the rest ignored.
#[derive(Deserialize)]
struct UserBody {
    email: EmailAddress,
}

/// user-service's `GET /users/{id}`.
#[derive(Debug, Clone)]
pub struct Users {
    client: ResilientClient,
    base_url: String,
}

impl Users {
    /// Calls `base_url` through `client`.
    #[must_use]
    pub fn new(base_url: &str, client: ResilientClient) -> Self {
        Self {
            client,
            base_url: base(base_url),
        }
    }

    /// The customer's address; `None` when user-service doesn't know them.
    ///
    /// # Errors
    /// A [`CallError`].
    pub async fn email(&self, customer: &CustomerId) -> Result<Option<EmailAddress>, CallError> {
        let found: Option<UserBody> = lookup(
            &self.client,
            &self.base_url,
            "user-service",
            "users",
            customer.as_str(),
        )
        .await?;
        Ok(found.map(|user| user.email))
    }
}

impl CustomerEmails for Users {
    async fn email(&self, customer: &CustomerId) -> Result<Option<EmailAddress>, LookupError> {
        Ok(Users::email(self, customer).await?)
    }
}

// The fields the Scala decoder needs; the rest ignored.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OrderBody {
    customer_id: CustomerId,
    guest_email: Option<EmailAddress>,
}

/// order-service's `GET /orders/{id}`.
#[derive(Debug, Clone)]
pub struct Orders {
    client: ResilientClient,
    base_url: String,
}

impl Orders {
    /// Calls `base_url` through `client`.
    #[must_use]
    pub fn new(base_url: &str, client: ResilientClient) -> Self {
        Self {
            client,
            base_url: base(base_url),
        }
    }

    /// Who the order belongs to; `None` when order-service doesn't know it.
    ///
    /// # Errors
    /// A [`CallError`].
    pub async fn contact(&self, order: &OrderId) -> Result<Option<OrderContact>, CallError> {
        let found: Option<OrderBody> = lookup(
            &self.client,
            &self.base_url,
            "order-service",
            "orders",
            order.as_str(),
        )
        .await?;
        Ok(found.map(|order| OrderContact {
            customer_id: order.customer_id,
            guest_email: order.guest_email,
        }))
    }
}

impl OrderContacts for Orders {
    async fn contact(&self, order: &OrderId) -> Result<Option<OrderContact>, LookupError> {
        Ok(Orders::contact(self, order).await?)
    }
}
