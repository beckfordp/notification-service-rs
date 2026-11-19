# Spec — could-fix: idiomatic types, a tested lifecycle

Track `rust-idioms-could-fix_20261118`, gluon **RPN-11-could-fix** (with
gluon-rs's `envelope-parity_20261118`); gluon TECHNICAL-DEBT.md items 13–15.
Idiomatic Rust over a straight Scala port; behaviour unchanged.

| Item | Change |
|---|---|
| 13 I4 | `FromStr` (and `Display`) for `RecipientType` and `EmailStatus`, replacing `parse() -> Option`; rows read through a `#[derive(sqlx::FromRow)]` row in `store.rs`, turned into `domain::SentEmail` by `TryFrom` (still `StoreError::UnknownValue` naming the column). `domain` stays free of sqlx. |
| 14 M1 | Newtypes `EventId`, `CustomerId`, `OrderId`, `EmailAddress` (no validation beyond Scala's: any text, as the Scala service takes it), and one `RecipientKey::{Customer, Email, Order}` replacing the (`recipient_type`, `recipient_key`) pair in `EmailMessage`, so a message can't name a customer by an address. The read model (`SentEmail`, `/emails`) keeps the columns as text: it echoes them. `UserRegistered` decodes straight into the newtypes. |
| 15 M2 | The run-until-stopped lifecycle (`main`'s `select!`, `ended_well`) moves to `lifecycle.rs`, generic over the two tasks and the stop signal, so its exits are tested: a consumer or server stopping by itself, or failing, fails the service and says so (Scala `KafkaConsumers.watched`). |

Acceptance: gluon `bin/verify-complete-RPN-11.sh` criteria 3–7.
