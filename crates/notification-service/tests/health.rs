// SPDX-License-Identifier: MIT

//! Liveness, the error shapes and shutdown, against the started binary
//! (capture `http/health.txt`, `health-head.txt`, `unknown-path.txt`).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use support::{Postgres, Service};

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.as_str())
}

#[test]
fn health_is_200_with_no_body_and_no_content_type() {
    let postgres = Postgres::start();
    let service = Service::start(&postgres.env());
    let (status, headers, body) = service.request("GET", "/health", &[]);
    assert_eq!(status, 200);
    assert_eq!(header(&headers, "content-length"), Some("0"));
    assert_eq!(header(&headers, "content-type"), None);
    assert_eq!(body, "");
}

#[test]
fn head_is_a_plain_text_405_without_allow() {
    let postgres = Postgres::start();
    let service = Service::start(&postgres.env());
    for path in ["/health", "/health/ready"] {
        let (status, headers, _) = service.request("HEAD", path, &[]);
        assert_eq!(status, 405, "{path}");
        assert_eq!(
            header(&headers, "content-type"),
            Some("text/plain; charset=UTF-8")
        );
        assert_eq!(header(&headers, "content-length"), Some("18"));
        assert_eq!(header(&headers, "allow"), None);
    }
    let (status, _, body) = service.request("POST", "/health", &[]);
    assert_eq!((status, body.as_str()), (405, "Method Not Allowed"));
}

#[test]
fn an_unknown_path_is_a_plain_text_404() {
    let postgres = Postgres::start();
    let service = Service::start(&postgres.env());
    let (status, headers, body) = service.request("GET", "/nope", &[]);
    assert_eq!((status, body.as_str()), (404, "Not found"));
    assert_eq!(
        header(&headers, "content-type"),
        Some("text/plain; charset=UTF-8")
    );
}

#[test]
fn sigterm_exits_0() {
    let postgres = Postgres::start();
    let mut service = Service::start(&postgres.env());
    service.terminate();
    assert_eq!(service.wait_exit().code(), Some(0));
    assert!(service.stdout().contains("notification-service starting"));
}

#[test]
fn bad_config_exits_1_listing_every_problem() {
    let mut service = Service::spawn(&[
        ("NOTIFICATION_SERVICE_PORT", "http".to_owned()),
        ("POSTGRES_PORT", "x".to_owned()),
    ]);
    assert_eq!(service.wait_exit().code(), Some(1));
    let stderr = service.stderr();
    assert!(stderr.contains("NOTIFICATION_SERVICE_PORT"), "{stderr}");
    assert!(stderr.contains("POSTGRES_PORT"), "{stderr}");
}
