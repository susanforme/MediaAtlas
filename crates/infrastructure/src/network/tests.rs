use super::*;
use mediaatlas_domain::{MediaKind, ProviderConfig, ProviderFailure, ProviderJob};
use serde_json::json;
use std::{
    future::Future,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    task::JoinHandle,
};
use uuid::Uuid;

struct MockServer {
    url: String,
    task: JoinHandle<()>,
}
impl Drop for MockServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}
async fn server<F, Fut>(handler: F) -> MockServer
where
    F: Fn(String) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = String> + Send + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let handler = Arc::new(handler);
    let task = tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            let handler = handler.clone();
            tokio::spawn(async move {
                let mut bytes = Vec::new();
                loop {
                    let mut buffer = [0_u8; 4096];
                    let count = socket.read(&mut buffer).await.unwrap_or_default();
                    if count == 0 {
                        return;
                    }
                    bytes.extend_from_slice(&buffer[..count]);
                    if let Some(end) = bytes.windows(4).position(|v| v == b"\r\n\r\n") {
                        let header = String::from_utf8_lossy(&bytes[..end]).to_ascii_lowercase();
                        let length = header
                            .lines()
                            .find_map(|line| {
                                line.strip_prefix("content-length: ")
                                    .and_then(|v| v.parse::<usize>().ok())
                            })
                            .unwrap_or_default();
                        if bytes.len() >= end + 4 + length {
                            break;
                        }
                    }
                    assert!(bytes.len() < 1024 * 1024);
                }
                let response = handler(String::from_utf8_lossy(&bytes).into_owned()).await;
                let _ = socket.write_all(response.as_bytes()).await;
                let _ = socket.shutdown().await;
            });
        }
    });
    MockServer {
        url: format!("http://{address}"),
        task,
    }
}
fn response(status: u16, content_type: &str, body: &str, headers: &str) -> String {
    format!("HTTP/1.1 {status} Fixture\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n{body}", body.len())
}
fn job(provider_id: &str) -> ProviderJob {
    ProviderJob {
        id: Uuid::new_v4(),
        provider_id: provider_id.to_owned(),
        media_item_id: None,
        media_kind: Some(MediaKind::Movie),
        file_identity: None,
        region: None,
        correlation_id: Uuid::new_v4(),
        operation: "search".into(),
        query: "Fixture title".into(),
        language: "en-US".into(),
        config: ProviderConfig {
            enabled: true,
            interval_ms: 0,
            max_retries: 0,
            timeout_ms: 3000,
            ..ProviderConfig::default()
        },
        created_at: chrono::Utc::now(),
    }
}
fn spec(url: &str) -> RequestSpec {
    RequestSpec::get(url, EngineKind::Api)
}
async fn failure(engine: &NetworkEngine, job: &ProviderJob, spec: RequestSpec) -> ProviderFailure {
    engine
        .execute(job, spec, &CancellationToken::new())
        .await
        .err()
        .expect("request must fail")
}

#[tokio::test]
async fn api_html_combination_graphql_pagination_and_extraction() {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let requests = captured.clone();
    let fixture = server(move |request| {
        requests.lock().unwrap().push(request.clone());
        async move {
            if request.starts_with("POST /graphql ") { response(200, "application/json", r#"{"data":{"id":"fixture-1"}}"#, "") }
            else if request.starts_with("GET /page2 ") { response(200, "application/json", r#"{"items":[2],"next":null}"#, "") }
            else if request.starts_with("GET /html ") { response(200, "text/html; charset=utf-8", r#"<title>Fixture</title><h1 class="name">Movie <span>Fixture</span></h1><meta property="og:title" content="Movie Fixture"><script type="application/ld+json">{"@type":"Movie","name":"Movie Fixture"}</script><script id="__DATA__" type="application/json">{"id":"fixture-1"}</script>"#, "") }
            else { response(200, "application/json", r#"{"items":[1],"next":"/page2"}"#, "") }
        }
    }).await;
    let engine = NetworkEngine::for_loopback(4);
    let job = job("fixture-combined");
    let pages = engine
        .json_pages(
            &job,
            spec(&format!("{}/page1", fixture.url)),
            "/next",
            3,
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(pages.len(), 2);
    assert_eq!(pages[1]["items"], json!([2]));
    let graphql = RequestSpec::graphql(
        format!("{}/graphql", fixture.url),
        "query Fixture($id: ID!) { movie(id: $id) { id } }",
        json!({"id":"fixture-1"}),
    )
    .unwrap();
    let result = engine
        .execute(&job, graphql, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(result.json().unwrap()["data"]["id"], "fixture-1");
    assert!(captured
        .lock()
        .unwrap()
        .iter()
        .any(|r| r.contains("\"variables\":{\"id\":\"fixture-1\"}")));
    let html = engine
        .execute(
            &job,
            RequestSpec::get(format!("{}/html", fixture.url), EngineKind::Html),
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    let document = parse_html(html.text().unwrap());
    assert_eq!(document.css_text("h1.name").unwrap(), vec!["Movie Fixture"]);
    assert_eq!(document.json_ld().unwrap()[0]["@type"], "Movie");
    assert_eq!(document.open_graph()["og:title"], "Movie Fixture");
    assert_eq!(
        document.embedded_json("__DATA__").unwrap()["id"],
        "fixture-1"
    );
    assert_eq!(
        document.embedded_json("missing").err(),
        Some(ProviderFailure::NotFound)
    );
    assert_eq!(
        document.css_text("[").err(),
        Some(ProviderFailure::ParseError)
    );
    assert!(
        RequestSpec::graphql("https://example.com", "mutation { deleteAll }", json!({})).is_err()
    );
}

#[tokio::test]
async fn response_failures_keep_classification_without_leaking_sensitive_payloads() {
    let fixture = server(|request| async move {
        let path = request.split_whitespace().nth(1).unwrap();
        let (status, body, kind) = match path {
            "/403" => (403, "blocked secret-token", "text/html"),
            "/404" => (404, "missing secret-token", "text/html"),
            "/401" => (401, "auth secret-token", "text/html"),
            "/500" => (500, "server secret-token", "text/html"),
            "/captcha" => (
                403,
                "<form class=\"g-recaptcha\">secret-token</form>",
                "text/html",
            ),
            "/json" => (200, "not json secret-token", "application/json"),
            "/mime" => (200, "{}", "application/octet-stream"),
            "/identity" => (200, r#"{"id":"wrong"}"#, "application/json"),
            _ => (200, "{}", "application/json"),
        };
        response(status, kind, body, "")
    })
    .await;
    let engine = NetworkEngine::for_loopback(4);
    for (path, expected) in [
        ("403", ProviderFailure::Blocked),
        ("404", ProviderFailure::NotFound),
        ("401", ProviderFailure::Http4xx),
        ("500", ProviderFailure::Http5xx),
        ("captcha", ProviderFailure::Captcha),
        ("json", ProviderFailure::ParseError),
        ("mime", ProviderFailure::ParseError),
    ] {
        assert_eq!(
            failure(
                &engine,
                &job(path),
                spec(&format!("{}/{path}", fixture.url))
            )
            .await,
            expected
        );
    }
    let mut identity = spec(&format!("{}/identity", fixture.url));
    identity.expected_identity = Some(("/id".into(), "expected".into()));
    assert_eq!(
        failure(&engine, &job("identity"), identity).await,
        ProviderFailure::IdentityMismatch
    );
    let records = serde_json::to_string(&engine.records().await).unwrap();
    assert!(!records.contains("secret-token"));
    assert!(!records.contains("/identity"));
    assert!(engine.records().await.iter().all(|r| r.failure.is_some()));
}

#[tokio::test]
async fn chunked_and_content_length_caps_reject_before_unbounded_body_allocation() {
    let fixture = server(|request| async move {
        if request.starts_with("GET /chunked ") {
            "HTTP/1.1 200 Fixture\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n8\r\n12345678\r\n8\r\n12345678\r\n0\r\n\r\n".into()
        } else { response(200, "application/json", "1234567890123456", "") }
    }).await;
    let engine = NetworkEngine::for_loopback(2);
    let mut job = job("caps");
    job.config.max_response_bytes = 12;
    for path in ["chunked", "length"] {
        assert_eq!(
            failure(&engine, &job, spec(&format!("{}/{path}", fixture.url))).await,
            ProviderFailure::ResponseTooLarge
        );
    }
}

#[tokio::test]
async fn redirects_remove_all_credentials_on_cross_origin_and_revalidate_target() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let observed_copy = observed.clone();
    let destination = server(move |request| {
        observed_copy.lock().unwrap().push(request);
        async { response(200, "application/json", "{}", "") }
    })
    .await;
    let target = destination.url.clone();
    let source = server(move |request| {
        let target = target.clone();
        async move {
            let location = if request.starts_with("GET /unsafe ") {
                "http://169.254.169.254/latest/meta-data".to_owned()
            } else {
                format!("{target}/detail")
            };
            response(302, "text/plain", "", &format!("Location: {location}\r\n"))
        }
    })
    .await;
    let engine = NetworkEngine::for_loopback(2);
    let job = job("redirect");
    let mut request = spec(&format!("{}/start?api_key=secret-token", source.url));
    request.bearer = Some(SecretString::new("secret-token".into()));
    request.cookie = Some(SecretString::new("session=secret-cookie".into()));
    request.api_key = Some(("x-api-key".into(), SecretString::new("secret-key".into())));
    engine
        .execute(&job, request, &CancellationToken::new())
        .await
        .unwrap();
    let captured = {
        let captured = observed.lock().unwrap();
        assert_eq!(captured.len(), 1);
        captured[0].to_ascii_lowercase()
    };
    for secret in [
        "authorization",
        "cookie",
        "x-api-key",
        "secret-token",
        "secret-cookie",
        "secret-key",
    ] {
        assert!(!captured.contains(secret), "redirect leaked {secret}");
    }
    assert_eq!(
        failure(&engine, &job, spec(&format!("{}/unsafe", source.url))).await,
        ProviderFailure::UnsafeTarget
    );
    let records = serde_json::to_string(&engine.records().await).unwrap();
    assert!(!records.contains("secret-token"));
    assert!(!records.contains("api_key"));
}

#[tokio::test]
async fn production_policy_rejects_private_ip_encodings_local_hosts_and_http() {
    let policy = super::policy::DestinationPolicy::default();
    for target in [
        "https://127.0.0.1",
        "https://2130706433",
        "https://0x7f000001",
        "https://127.1",
        "https://10.0.0.1",
        "https://172.16.0.1",
        "https://192.168.1.1",
        "https://100.64.0.1",
        "https://169.254.169.254",
        "https://[::1]",
        "https://[::ffff:127.0.0.1]",
        "https://[fc00::1]",
        "https://[fe80::1]",
        "https://metadata.google.internal",
        "https://foo.localhost.",
        "http://example.com",
        "https://secret@example.com",
        "https://example.com/#secret",
        "file:///etc/passwd",
    ] {
        let url = reqwest::Url::parse(target).unwrap();
        assert_eq!(
            policy.validate_url(&url),
            Err(ProviderFailure::UnsafeTarget),
            "target: {target}"
        );
    }
    assert!(policy
        .validate_url(&reqwest::Url::parse("https://example.com").unwrap())
        .is_ok());
    assert_eq!(
        safe_origin("https://user:password@example.com/private?token=secret#hidden"),
        "https://example.com"
    );
    for ip in ["8.8.8.8", "1.1.1.1", "2606:4700:4700::1111"] {
        assert!(is_public_ip(ip.parse().unwrap()));
    }
    for ip in [
        "0.0.0.0",
        "192.0.2.1",
        "198.18.0.1",
        "198.51.100.1",
        "203.0.113.1",
        "224.0.0.1",
        "255.255.255.255",
        "2001:db8::1",
        "2002:7f00:1::",
        "64:ff9b::7f00:1",
    ] {
        assert!(!is_public_ip(ip.parse().unwrap()), "ip: {ip}");
    }
}

#[tokio::test]
async fn timeout_and_inflight_cancellation_release_capacity_and_record_safe_failure() {
    let started = Arc::new(tokio::sync::Notify::new());
    let started_copy = started.clone();
    let fixture = server(move |request| {
        let started = started_copy.clone();
        async move {
            if request.starts_with("GET /slow ") {
                started.notify_one();
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            response(200, "application/json", "{}", "")
        }
    })
    .await;
    let engine = NetworkEngine::for_loopback(1);
    let mut timeout_job = job("timeout");
    timeout_job.config.timeout_ms = 40;
    assert_eq!(
        failure(
            &engine,
            &timeout_job,
            spec(&format!("{}/slow", fixture.url))
        )
        .await,
        ProviderFailure::Timeout
    );
    let cancel = CancellationToken::new();
    let cloned_cancel = cancel.clone();
    let cloned_engine = engine.clone();
    let slow_url = format!("{}/slow", fixture.url);
    let task = tokio::spawn(async move {
        cloned_engine
            .execute(&job("cancel"), spec(&slow_url), &cloned_cancel)
            .await
            .err()
    });
    started.notified().await;
    tokio::time::sleep(Duration::from_millis(10)).await;
    cancel.cancel();
    assert_eq!(task.await.unwrap(), Some(ProviderFailure::Cancelled));
    engine
        .execute(
            &job("healthy"),
            spec(&fixture.url),
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    let records = engine.records().await;
    assert!(records
        .iter()
        .any(|r| r.failure == Some(ProviderFailure::Timeout)));
    assert!(records
        .iter()
        .any(|r| r.failure == Some(ProviderFailure::Cancelled)));
}

#[tokio::test]
async fn semaphore_and_rate_waits_are_cancellable_without_dispatching_requests() {
    let count = Arc::new(AtomicUsize::new(0));
    let seen = count.clone();
    let entered = Arc::new(tokio::sync::Notify::new());
    let notify = entered.clone();
    let fixture = server(move |request| {
        seen.fetch_add(1, Ordering::SeqCst);
        let notify = notify.clone();
        async move {
            if request.starts_with("GET /hold ") {
                notify.notify_one();
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
            response(200, "application/json", "{}", "")
        }
    })
    .await;
    let engine = NetworkEngine::for_loopback(1);
    let running_engine = engine.clone();
    let hold_url = format!("{}/hold", fixture.url);
    let running = tokio::spawn(async move {
        running_engine
            .execute(&job("occupier"), spec(&hold_url), &CancellationToken::new())
            .await
            .is_ok()
    });
    entered.notified().await;
    let cancel = CancellationToken::new();
    let cancel_copy = cancel.clone();
    let waiting_engine = engine.clone();
    let url = fixture.url.clone();
    let waiting = tokio::spawn(async move {
        waiting_engine
            .execute(&job("waiting"), spec(&url), &cancel_copy)
            .await
            .err()
    });
    tokio::time::sleep(Duration::from_millis(20)).await;
    cancel.cancel();
    assert_eq!(waiting.await.unwrap(), Some(ProviderFailure::Cancelled));
    assert!(running.await.unwrap());
    assert_eq!(count.load(Ordering::SeqCst), 1);
    let mut limited = job("rate");
    limited.config.interval_ms = 1000;
    engine
        .execute(&limited, spec(&fixture.url), &CancellationToken::new())
        .await
        .unwrap();
    let cancel = CancellationToken::new();
    let copy = cancel.clone();
    let e = engine.clone();
    let url = fixture.url.clone();
    let waiting = tokio::spawn(async move { e.execute(&limited, spec(&url), &copy).await.err() });
    tokio::time::sleep(Duration::from_millis(20)).await;
    cancel.cancel();
    assert_eq!(waiting.await.unwrap(), Some(ProviderFailure::Cancelled));
    assert_eq!(count.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn retry_after_cools_only_failed_provider_and_allows_single_recovery_probe() {
    let count = Arc::new(AtomicUsize::new(0));
    let seen = count.clone();
    let fixture = server(move |_| {
        let attempt = seen.fetch_add(1, Ordering::SeqCst);
        async move {
            if attempt == 0 {
                response(429, "application/json", "{}", "Retry-After: 1\r\n")
            } else {
                response(200, "application/json", "{}", "")
            }
        }
    })
    .await;
    let engine = NetworkEngine::for_loopback(1);
    let limited = job("rate-limited");
    assert_eq!(
        failure(&engine, &limited, spec(&fixture.url)).await,
        ProviderFailure::RateLimited
    );
    assert_eq!(engine.health("rate-limited").await, "cooling_down");
    assert_eq!(
        failure(&engine, &limited, spec(&fixture.url)).await,
        ProviderFailure::CoolingDown
    );
    let before = Instant::now();
    engine
        .execute(
            &job("healthy-other-provider"),
            spec(&fixture.url),
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(before.elapsed() < Duration::from_millis(300));
    tokio::time::sleep(Duration::from_millis(1050)).await;
    assert_eq!(engine.health("rate-limited").await, "probe_ready");
    engine
        .execute(&limited, spec(&fixture.url), &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(engine.health("rate-limited").await, "healthy");
}

#[tokio::test]
async fn retry_backoff_does_not_hold_global_capacity_against_healthy_provider() {
    let attempted = Arc::new(tokio::sync::Notify::new());
    let notify = attempted.clone();
    let count = Arc::new(AtomicUsize::new(0));
    let seen = count.clone();
    let fixture = server(move |request| {
        let notify = notify.clone();
        let n = seen.fetch_add(1, Ordering::SeqCst);
        async move {
            if request.starts_with("GET /limited ") && n == 0 {
                notify.notify_one();
                response(429, "application/json", "{}", "Retry-After: 1\r\n")
            } else {
                response(200, "application/json", "{}", "")
            }
        }
    })
    .await;
    let engine = NetworkEngine::for_loopback(1);
    let mut limited = job("retrying-provider");
    limited.config.max_retries = 1;
    let e = engine.clone();
    let url = format!("{}/limited", fixture.url);
    let retrying = tokio::spawn(async move {
        e.execute(&limited, spec(&url), &CancellationToken::new())
            .await
            .is_ok()
    });
    attempted.notified().await;
    let result = tokio::time::timeout(
        Duration::from_millis(300),
        engine.execute(
            &job("unrelated-provider"),
            spec(&fixture.url),
            &CancellationToken::new(),
        ),
    )
    .await;
    assert!(
        result.is_ok(),
        "a provider's retry backoff monopolized the global permit"
    );
    assert!(result.unwrap().is_ok());
    assert!(retrying.await.unwrap());
}

#[tokio::test]
async fn provider_concurrency_limits_apply_across_jobs_and_image_downloads() {
    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let a = active.clone();
    let p = peak.clone();
    let fixture = server(move |request| {
        let a = a.clone();
        let p = p.clone();
        async move {
            let now = a.fetch_add(1, Ordering::SeqCst) + 1;
            p.fetch_max(now, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_millis(30)).await;
            a.fetch_sub(1, Ordering::SeqCst);
            if request.starts_with("GET /image ") {
                response(200, "image/webp", "RIFF0000WEBP", "")
            } else {
                response(200, "application/json", "{}", "")
            }
        }
    })
    .await;
    let engine = NetworkEngine::for_loopback(4);
    let mut tasks = Vec::new();
    for _ in 0..6 {
        let e = engine.clone();
        let url = fixture.url.clone();
        let mut j = job("shared-provider");
        j.config.concurrency = 2;
        tasks.push(tokio::spawn(async move {
            e.execute(&j, spec(&url), &CancellationToken::new())
                .await
                .is_ok()
        }));
    }
    for task in tasks {
        assert!(task.await.unwrap());
    }
    assert_eq!(peak.load(Ordering::SeqCst), 2);
    peak.store(0, Ordering::SeqCst);
    let mut tasks = Vec::new();
    for _ in 0..4 {
        let e = engine.clone();
        let url = format!("{}/image", fixture.url);
        let mut j = job("image-provider");
        j.config.concurrency = 4;
        j.config.image_concurrency = 1;
        tasks.push(tokio::spawn(async move {
            e.execute(
                &j,
                RequestSpec::get(url, EngineKind::Image),
                &CancellationToken::new(),
            )
            .await
            .is_ok()
        }));
    }
    for task in tasks {
        assert!(task.await.unwrap());
    }
    assert_eq!(peak.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn pagination_rejects_cycles_cross_origin_and_non_url_cursor() {
    let fixture = server(|request| async move {
        let body = if request.starts_with("GET /cross ") {
            r#"{"next":"https://example.com/path?token=secret"}"#
        } else if request.starts_with("GET /invalid ") {
            r#"{"next":42}"#
        } else {
            r#"{"next":"/cycle"}"#
        };
        response(200, "application/json", body, "")
    })
    .await;
    let engine = NetworkEngine::for_loopback(2);
    for (path, expected) in [
        ("cross", ProviderFailure::UnsafeTarget),
        ("invalid", ProviderFailure::ParseError),
        ("cycle", ProviderFailure::ParseError),
    ] {
        assert_eq!(
            engine
                .json_pages(
                    &job("pages"),
                    spec(&format!("{}/{path}", fixture.url)),
                    "/next",
                    4,
                    &CancellationToken::new()
                )
                .await
                .err(),
            Some(expected)
        );
    }
}

#[tokio::test]
async fn hostname_resolution_rejects_private_addresses_before_connecting() {
    // Uses the platform hosts resolver, not a hard-coded is_public_ip assertion.
    // localhost is required to resolve locally and cannot reach an external site.
    let policy = super::policy::DestinationPolicy::default();
    assert_eq!(
        policy.resolve("localhost", 443).await.err(),
        Some(ProviderFailure::UnsafeTarget)
    );
}

#[tokio::test]
async fn cancelled_half_open_probe_releases_probe_exclusivity() {
    let started = Arc::new(tokio::sync::Notify::new());
    let notify = started.clone();
    let requests = Arc::new(AtomicUsize::new(0));
    let count = requests.clone();
    let fixture = server(move |request| {
        let notify = notify.clone();
        let n = count.fetch_add(1, Ordering::SeqCst);
        async move {
            if n == 0 {
                return response(429, "application/json", "{}", "Retry-After: 0\r\n");
            }
            if request.starts_with("GET /probe ") {
                notify.notify_one();
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            response(200, "application/json", "{}", "")
        }
    })
    .await;
    let engine = NetworkEngine::for_loopback(2);
    let limited = job("half-open");
    assert_eq!(
        failure(&engine, &limited, spec(&fixture.url)).await,
        ProviderFailure::RateLimited
    );
    tokio::time::sleep(Duration::from_millis(550)).await;
    let cancel = CancellationToken::new();
    let copy = cancel.clone();
    let e = engine.clone();
    let j = limited.clone();
    let url = format!("{}/probe", fixture.url);
    let probe = tokio::spawn(async move { e.execute(&j, spec(&url), &copy).await.err() });
    started.notified().await;
    assert_eq!(
        failure(&engine, &limited, spec(&fixture.url)).await,
        ProviderFailure::CoolingDown
    );
    cancel.cancel();
    assert_eq!(probe.await.unwrap(), Some(ProviderFailure::Cancelled));
    // Cancellation must not leave the provider permanently stuck in probe=true.
    tokio::task::yield_now().await;
    engine
        .execute(&limited, spec(&fixture.url), &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(engine.health("half-open").await, "healthy");
}

#[tokio::test]
async fn retries_observe_provider_request_interval() {
    let times = Arc::new(Mutex::new(Vec::new()));
    let captured = times.clone();
    let fixture = server(move |_| {
        let count = {
            let mut times = captured.lock().unwrap();
            times.push(Instant::now());
            times.len()
        };
        async move {
            response(
                if count == 1 { 500 } else { 200 },
                "application/json",
                "{}",
                "",
            )
        }
    })
    .await;
    let engine = NetworkEngine::for_loopback(2);
    let mut retrying = job("retry-interval");
    retrying.config.max_retries = 1;
    retrying.config.interval_ms = 180;
    engine
        .execute(&retrying, spec(&fixture.url), &CancellationToken::new())
        .await
        .unwrap();
    let times = times.lock().unwrap();
    assert_eq!(times.len(), 2);
    assert!(
        times[1].duration_since(times[0]) >= Duration::from_millis(165),
        "retry bypassed rate interval"
    );
}

#[tokio::test]
async fn connection_refusal_is_network_error_without_retrying_non_idempotent_post() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let engine = NetworkEngine::for_loopback(2);
    let mut j = job("connection-refused");
    j.config.max_retries = 3;
    let request =
        RequestSpec::graphql(format!("http://{address}"), "query { fixture }", json!({})).unwrap();
    assert_eq!(
        failure(&engine, &j, request).await,
        ProviderFailure::NetworkError
    );
    let records = engine.records().await;
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].attempt, 1);
}

#[tokio::test]
async fn tls_rejects_untrusted_self_signed_mock_certificate() {
    use tokio_rustls::{
        rustls::{self, pki_types::PrivatePkcs8KeyDer},
        TlsAcceptor,
    };
    let certified = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let key = PrivatePkcs8KeyDer::from(certified.signing_key.serialize_der());
    let tls = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_no_client_auth()
    .with_single_cert(vec![certified.cert.der().clone()], key.into())
    .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        if let Ok((socket, _)) = listener.accept().await {
            let _ = TlsAcceptor::from(Arc::new(tls)).accept(socket).await;
        }
    });
    let engine = NetworkEngine::for_loopback(1);
    assert_eq!(
        failure(
            &engine,
            &job("untrusted-tls"),
            spec(&format!("https://{address}"))
        )
        .await,
        ProviderFailure::TlsError
    );
    task.await.unwrap();
    let records = engine.records().await;
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].failure, Some(ProviderFailure::TlsError));
}

#[tokio::test]
async fn resolver_failure_is_dns_error_and_does_not_dispatch_http() {
    struct FailingResolver;
    impl reqwest::dns::Resolve for FailingResolver {
        fn resolve(&self, _: reqwest::dns::Name) -> reqwest::dns::Resolving {
            Box::pin(async {
                Err(Box::new(ProviderFailure::DnsError) as Box<dyn std::error::Error + Send + Sync>)
            })
        }
    }
    let engine = NetworkEngine::for_test_resolver(Arc::new(FailingResolver));
    let j = job("dns-failure");
    assert_eq!(
        failure(&engine, &j, spec("https://fixture.example")).await,
        ProviderFailure::DnsError
    );
    let records = engine.records().await;
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].status, None);
    assert_eq!(records[0].failure, Some(ProviderFailure::DnsError));
}

#[tokio::test]
async fn enormous_retry_after_cannot_overflow_into_an_immediate_retry() {
    let fixture = server(|_| async {
        response(
            429,
            "application/json",
            "{}",
            "Retry-After: 18446744073709551615\r\n",
        )
    })
    .await;
    let engine = NetworkEngine::for_loopback(1);
    let j = job("long-cooldown");
    assert_eq!(
        failure(&engine, &j, spec(&fixture.url)).await,
        ProviderFailure::RateLimited
    );
    assert_eq!(engine.health("long-cooldown").await, "cooling_down");
    assert_eq!(
        failure(&engine, &j, spec(&fixture.url)).await,
        ProviderFailure::CoolingDown
    );
}

#[tokio::test]
async fn concurrent_late_success_cannot_clear_another_requests_active_cooldown() {
    let late_entered = Arc::new(tokio::sync::Notify::new());
    let entered = late_entered.clone();
    let fixture = server(move |request| {
        let entered = entered.clone();
        async move {
            if request.starts_with("GET /late ") {
                entered.notify_one();
                tokio::time::sleep(Duration::from_millis(100)).await;
                response(200, "application/json", "{}", "")
            } else {
                response(429, "application/json", "{}", "Retry-After: 1\r\n")
            }
        }
    })
    .await;
    let engine = NetworkEngine::for_loopback(2);
    let j = job("concurrent-cooldown");
    let e = engine.clone();
    let late_job = j.clone();
    let url = format!("{}/late", fixture.url);
    let late = tokio::spawn(async move {
        e.execute(&late_job, spec(&url), &CancellationToken::new())
            .await
            .is_ok()
    });
    late_entered.notified().await;
    assert_eq!(
        failure(&engine, &j, spec(&format!("{}/rate", fixture.url))).await,
        ProviderFailure::RateLimited
    );
    assert!(late.await.unwrap());
    assert_eq!(engine.health("concurrent-cooldown").await, "cooling_down");
    assert_eq!(
        failure(&engine, &j, spec(&fixture.url)).await,
        ProviderFailure::CoolingDown
    );
}

#[tokio::test]
async fn global_queue_wait_does_not_spend_provider_interval_before_dispatch() {
    let occupied = Arc::new(tokio::sync::Notify::new());
    let entered = occupied.clone();
    let times = Arc::new(Mutex::new(Vec::new()));
    let arrivals = times.clone();
    let fixture = server(move |request| {
        let entered = entered.clone();
        let arrivals = arrivals.clone();
        async move {
            if request.starts_with("GET /occupy ") {
                entered.notify_one();
                tokio::time::sleep(Duration::from_millis(200)).await;
            } else {
                arrivals.lock().unwrap().push(Instant::now());
            }
            response(200, "application/json", "{}", "")
        }
    })
    .await;
    let engine = NetworkEngine::for_loopback(1);
    let e = engine.clone();
    let url = format!("{}/occupy", fixture.url);
    let occupying = tokio::spawn(async move {
        e.execute(
            &job("third-provider"),
            spec(&url),
            &CancellationToken::new(),
        )
        .await
        .is_ok()
    });
    occupied.notified().await;
    let mut waiting = Vec::new();
    for _ in 0..2 {
        let e = engine.clone();
        let url = fixture.url.clone();
        let mut j = job("rate-budget");
        j.config.interval_ms = 120;
        waiting.push(tokio::spawn(async move {
            e.execute(&j, spec(&url), &CancellationToken::new())
                .await
                .is_ok()
        }));
    }
    assert!(occupying.await.unwrap());
    for task in waiting {
        assert!(task.await.unwrap());
    }
    let times = times.lock().unwrap();
    assert_eq!(times.len(), 2);
    assert!(
        times[1].duration_since(times[0]) >= Duration::from_millis(105),
        "provider requests bunched after global queue wait"
    );
}

#[tokio::test]
async fn secret_query_is_sent_only_to_origin_and_cross_origin_reflection_is_rejected() {
    let target_calls = Arc::new(AtomicUsize::new(0));
    let seen = target_calls.clone();
    let target = server(move |_| {
        seen.fetch_add(1, Ordering::SeqCst);
        async { response(200, "application/json", r#"{"ok":true}"#, "") }
    })
    .await;
    let target_url = target.url.clone();
    let captured = Arc::new(Mutex::new(Vec::new()));
    let records = captured.clone();
    let origin = server(move |request| {
        records.lock().unwrap().push(request.clone());
        let target_url = target_url.clone();
        async move {
            if request.starts_with("GET /redirect?") {
                response(
                    302,
                    "application/json",
                    "{}",
                    &format!("Location: {target_url}/sink?api_key=synthetic-query-secret\r\n"),
                )
            } else {
                response(200, "application/json", r#"{"ok":true}"#, "")
            }
        }
    })
    .await;
    let engine = NetworkEngine::for_loopback(2);
    let job = job("secret-query");
    let mut request = spec(&format!("{}/data", origin.url));
    request.secret_query = Some((
        "api_key".into(),
        SecretString::new("synthetic-query-secret".into()),
    ));
    engine
        .execute(&job, request, &CancellationToken::new())
        .await
        .unwrap();
    assert!(captured.lock().unwrap()[0].starts_with("GET /data?api_key=synthetic-query-secret "));
    let mut request = spec(&format!("{}/redirect", origin.url));
    request.secret_query = Some((
        "api_key".into(),
        SecretString::new("synthetic-query-secret".into()),
    ));
    assert_eq!(
        failure(&engine, &job, request).await,
        ProviderFailure::UnsafeTarget
    );
    assert_eq!(target_calls.load(Ordering::SeqCst), 0);
    assert!(!serde_json::to_string(&engine.records().await)
        .unwrap()
        .contains("synthetic-query-secret"));
}
