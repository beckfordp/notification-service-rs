// SPDX-License-Identifier: MIT

//! The recipient lookups (Scala `RecipientResolver.apply`; ADM-3, ADR 0025),
//! behind [`CarriedAddress`](crate::email::CarriedAddress): a message naming
//! its customer asks user-service for the address; one naming only its order
//! asks order-service whose it is first. A customer user-service doesn't know
//! falls back to the order's guest address. No address at all is
//! [`Recipient::Unknown`] (`unknown_order:…`, `unknown_customer:…`,
//! `no_customer_identity`); a failed lookup is a [`LookupError`].

use std::future::Future;

use crate::domain::{CustomerId, EmailAddress, EmailMessage, OrderContact, OrderId};
use crate::email::{LookupError, Recipient, Resolver};

/// A customer's address (user-service; Scala `UserClient`).
pub trait CustomerEmails: Send + Sync {
    /// The address; `None` when nobody knows the customer.
    ///
    /// # Errors
    /// A failed lookup.
    fn email(
        &self,
        customer: &CustomerId,
    ) -> impl Future<Output = Result<Option<EmailAddress>, LookupError>> + Send;
}

/// Whose an order is (order-service; Scala `OrderClient`).
pub trait OrderContacts: Send + Sync {
    /// Its customer and guest address; `None` when nobody knows the order.
    ///
    /// # Errors
    /// A failed lookup.
    fn contact(
        &self,
        order: &OrderId,
    ) -> impl Future<Output = Result<Option<OrderContact>, LookupError>> + Send;
}

/// Finds an address through `users` and `orders`.
#[derive(Debug, Clone)]
pub struct Lookups<U, O> {
    users: U,
    orders: O,
}

impl<U, O> Lookups<U, O> {
    /// Looks up through `users` and `orders`.
    #[must_use]
    pub fn new(users: U, orders: O) -> Self {
        Self { users, orders }
    }
}

impl<U: CustomerEmails, O: OrderContacts> Lookups<U, O> {
    /// The customer to look up, and the order's contact if it took a lookup to
    /// find them (Scala `customerFor`); `Err` the unresolved reason.
    async fn customer(
        &self,
        message: &EmailMessage,
    ) -> Result<Result<(CustomerId, Option<OrderContact>), String>, LookupError> {
        if let Some(customer) = &message.customer_id {
            return Ok(Ok((customer.clone(), None)));
        }
        let Some(order) = &message.order_id else {
            return Ok(Err("no_customer_identity".to_owned()));
        };
        Ok(match self.orders.contact(order).await? {
            Some(contact) => Ok((contact.customer_id.clone(), Some(contact))),
            None => Err(format!("unknown_order:{order}")),
        })
    }

    /// The order's guest address: from the contact already found, else asked
    /// for (Scala `guestEmail`).
    async fn guest_email(
        &self,
        message: &EmailMessage,
        contact: Option<OrderContact>,
    ) -> Result<Option<EmailAddress>, LookupError> {
        if let Some(contact) = contact {
            return Ok(contact.guest_email);
        }
        let Some(order) = &message.order_id else {
            return Ok(None);
        };
        Ok(self
            .orders
            .contact(order)
            .await?
            .and_then(|contact| contact.guest_email))
    }
}

impl<U: CustomerEmails, O: OrderContacts> Resolver for Lookups<U, O> {
    async fn resolve(&self, message: &EmailMessage) -> Result<Recipient, LookupError> {
        let (customer, contact) = match self.customer(message).await? {
            Ok(found) => found,
            Err(reason) => return Ok(Recipient::Unknown(reason)),
        };
        let address = match self.users.email(&customer).await? {
            Some(address) => Some(address),
            None => self.guest_email(message, contact).await?,
        };
        Ok(match address {
            Some(address) => Recipient::Found {
                address,
                customer_id: Some(customer),
            },
            None => Recipient::Unknown(format!("unknown_customer:{customer}")),
        })
    }
}
