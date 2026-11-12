// SPDX-License-Identifier: MIT

//! The Scala service's V1/V2 migrations, applied at startup.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use support::{Postgres, Service, free_port};

#[test]
fn startup_migrates_sent_email_and_a_restart_reuses_it() {
    let postgres = Postgres::start();
    let service = Service::start(&postgres.env());
    assert_eq!(
        postgres
            .query(
                "select string_agg(table_name, ',' order by table_name) \
                 from information_schema.tables where table_schema = 'public'"
            )
            .trim(),
        "_sqlx_migrations,sent_email"
    );
    assert_eq!(
        postgres
            .query(
                "select string_agg(column_name || ':' || data_type, ',' order by ordinal_position) \
                 from information_schema.columns where table_name = 'sent_email'"
            )
            .trim(),
        "id:uuid,source_event_id:text,event_type:text,reason:text,recipient_type:text,\
         recipient_key:text,customer_id:text,order_id:text,subject:text,body:text,status:text,\
         created_at:timestamp with time zone,sent_at:timestamp with time zone,\
         failed_at:timestamp with time zone,error:text,recipient_email:text"
    );
    assert_eq!(
        postgres
            .query(
                "select pg_get_constraintdef(oid) from pg_constraint \
                 where conname = 'sent_email_status_check'"
            )
            .trim(),
        "CHECK ((status = ANY (ARRAY['pending'::text, 'sent'::text, 'failed'::text, 'unresolved'::text])))"
    );
    drop(service);
    let again = Service::start(&postgres.env());
    assert_eq!(again.get("/health/ready").unwrap().0, 200);
}

#[test]
fn an_unreachable_database_at_startup_exits_1() {
    let mut service = Service::spawn(&[
        ("POSTGRES_HOST", "127.0.0.1".to_owned()),
        ("POSTGRES_PORT", free_port().to_string()),
    ]);
    assert_eq!(service.wait_exit().code(), Some(1));
    let stderr = service.stderr();
    assert!(stderr.contains("database"), "{stderr}");
}
