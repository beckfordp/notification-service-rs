Files from capture `contracts/notification-service/20261111` (gluon-contract,
`tests/contract`), copied unchanged: `db/schema.sql`. `tests/capture.rs` compares the
service's `sent_email` DDL with it. Replace it only with a new capture. The health
exchanges it once held are checked by the harness (gluon TD-42).
