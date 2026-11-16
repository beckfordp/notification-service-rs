// SPDX-License-Identifier: MIT

//! CORS as the Scala service answers it: http4s 0.23 `CORS.policy` with its
//! `Cors.scala` settings (any origin; GET, POST, PATCH, PUT, DELETE;
//! `Content-Type`; no credentials; nothing exposed), outermost, around every
//! route (`AppRoutes`). Captures `http/cors-preflight.txt`,
//! `cors-preflight-bad-header.txt`, `cors-simple.txt`; the cases they don't
//! show follow http4s's `CORSPolicy` source.

use axum::extract::Request;
use axum::http::header::{
    ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_METHODS, ACCESS_CONTROL_ALLOW_ORIGIN,
    ACCESS_CONTROL_REQUEST_HEADERS, ACCESS_CONTROL_REQUEST_METHOD, ORIGIN, VARY,
};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

/// The methods a preflight may ask for (`Cors.scala`), as http4s lists them.
const ALLOWED_METHODS: [Method; 5] = [
    Method::PATCH,
    Method::PUT,
    Method::GET,
    Method::POST,
    Method::DELETE,
];

/// `Access-Control-Allow-Methods`, in the capture's order.
const ALLOW_METHODS: &str = "PATCH, PUT, GET, POST, DELETE";

/// The one request header a preflight may ask for, compared case-insensitively.
const ALLOWED_HEADER: &str = "Content-Type";

/// http4s's `Vary` on every OPTIONS answer: the policy lists methods and
/// headers, and allows any origin (so not `Origin`).
const VARY_OPTIONS: &str = "Access-Control-Request-Method, Access-Control-Request-Headers";

/// Marks a response no route produced (the 404 fallback): http4s's CORS
/// wraps the routes, so it never sees `orNotFound`'s 404.
#[derive(Debug, Clone, Copy)]
pub struct Unrouted;

/// The middleware.
pub async fn cors(request: Request, next: Next) -> Response {
    let headers = request.headers();
    let has_origin = headers.contains_key(ORIGIN);
    let is_options = request.method() == Method::OPTIONS;
    if has_origin
        && is_options
        && let Some(method) = requested_method(headers)
    {
        return preflight(allows(&method, &requested_headers(headers)));
    }
    let mut response = next.run(request).await;
    if response.extensions().get::<Unrouted>().is_some() {
        return response;
    }
    if has_origin {
        response
            .headers_mut()
            .insert(ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"));
    }
    if is_options {
        add_vary(response.headers_mut());
    }
    response
}

/// A preflight's answer, on any path: 200, empty; the `Access-Control-Allow-*`
/// headers only when the policy allows the request (capture
/// `cors-preflight-bad-header.txt`: only `Vary` otherwise).
fn preflight(allowed: bool) -> Response {
    let mut response = StatusCode::OK.into_response();
    let headers = response.headers_mut();
    if allowed {
        headers.insert(ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"));
        headers.insert(
            ACCESS_CONTROL_ALLOW_METHODS,
            HeaderValue::from_static(ALLOW_METHODS),
        );
        headers.insert(
            ACCESS_CONTROL_ALLOW_HEADERS,
            HeaderValue::from_static(ALLOWED_HEADER),
        );
    }
    add_vary(headers);
    response
}

/// http4s appends its `Vary` to one already there.
fn add_vary(headers: &mut HeaderMap) {
    let vary = match headers.get(VARY).and_then(|old| old.to_str().ok()) {
        Some(old) => HeaderValue::from_str(&format!("{old}, {VARY_OPTIONS}"))
            .unwrap_or(HeaderValue::from_static(VARY_OPTIONS)),
        None => HeaderValue::from_static(VARY_OPTIONS),
    };
    headers.insert(VARY, vary);
}

/// `Access-Control-Request-Method` as a method; `None` when absent or not a
/// method token (http4s then doesn't see a preflight).
fn requested_method(headers: &HeaderMap) -> Option<Method> {
    let value = headers.get(ACCESS_CONTROL_REQUEST_METHOD)?;
    Method::from_bytes(value.as_bytes()).ok()
}

/// Every `Access-Control-Request-Headers` name, split as http4s splits each
/// header (Java's `split("\\s*,\\s*")`).
fn requested_headers(headers: &HeaderMap) -> Vec<String> {
    headers
        .get_all(ACCESS_CONTROL_REQUEST_HEADERS)
        .iter()
        .flat_map(|value| split_names(&String::from_utf8_lossy(value.as_bytes())))
        .collect()
}

/// Whether the policy allows a preflight for `method` with `headers`: the
/// method is one of the five, and every header is `Content-Type` (an empty
/// name isn't).
#[must_use]
pub fn allows(method: &Method, headers: &[String]) -> bool {
    ALLOWED_METHODS.contains(method)
        && headers
            .iter()
            .all(|name| name.eq_ignore_ascii_case(ALLOWED_HEADER))
}

/// A header list split as Java's `String.split("\\s*,\\s*")`: no comma is the
/// whole value; otherwise the trimmed pieces, trailing empty ones dropped.
#[must_use]
pub fn split_names(value: &str) -> Vec<String> {
    let value = value.trim();
    if !value.contains(',') {
        return vec![value.to_owned()];
    }
    let mut names: Vec<String> = value
        .split(',')
        .map(|name| name.trim().to_owned())
        .collect();
    while names.last().is_some_and(String::is_empty) {
        names.pop();
    }
    names
}
