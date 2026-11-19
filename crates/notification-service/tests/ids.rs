// SPDX-License-Identifier: MIT

//! Row ids: injected through `IdGen` (rust.md §5), random in the service.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use notification_service::ids::{IdGen, RandomIds};

#[test]
fn random_ids_are_version_4_and_fresh() {
    let (first, second) = (RandomIds.next_id(), RandomIds.next_id());

    assert_eq!(first.get_version_num(), 4);
    assert_ne!(first, second);
}
