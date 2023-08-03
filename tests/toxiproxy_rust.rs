#![deny(warnings)]

use std::net::TcpListener;
use std::net::TcpStream;
use std::sync::Arc;
use std::thread::spawn;
use std::time::SystemTime;
use std::{io::prelude::*, time::Duration};

use axum::routing::get;
use axum::Router;
use proxy::*;
use tokio::sync::Mutex;
use toxiproxy_rust::toxic::ToxicCondition;
use toxiproxy_rust::*;

/**
 * WARNING!!!: This test depends on Toxiproxy (https://github.com/Shopify/toxiproxy) server running locally on default port.
 */

#[test]
fn test_is_running() {
    assert!(TOXIPROXY.is_running());
}

#[test]
fn test_reset() {
    assert!(TOXIPROXY.reset().is_ok());
}

#[test]
fn test_populate() {
    let result = TOXIPROXY.populate(vec![ProxyPack::new(
        "socket".into(),
        "localhost:2001".into(),
        "localhost:2000".into(),
    )]);

    assert!(result.is_ok());

    assert_eq!(1, result.as_ref().unwrap().len());
    assert_eq!("socket", result.as_ref().unwrap()[0].proxy_pack.name);
}

#[test]
fn test_all() {
    populate_example();

    let result = TOXIPROXY.all();
    assert!(result.is_ok());
    assert_eq!(1, result.as_ref().unwrap().len());
}

#[test]
fn test_version() {
    assert!(TOXIPROXY.version().is_ok());
}

#[test]
fn test_find_and_reset_proxy() {
    populate_example();

    let result = TOXIPROXY.find_and_reset_proxy("socket");
    assert!(result.is_ok());

    assert_eq!("socket", result.as_ref().unwrap().proxy_pack.name);
}

#[test]
fn test_find_and_reset_proxy_invalid() {
    let result = TOXIPROXY.find_and_reset_proxy("bad-proxy");
    assert!(result.is_err());
}

#[test]
fn test_proxy_down() {
    populate_example();

    let result = TOXIPROXY.find_and_reset_proxy("socket");
    assert!(result.is_ok());
    assert!(result.as_ref().unwrap().proxy_pack.enabled);

    assert!(result
        .as_ref()
        .unwrap()
        .with_down(|| {
            let result = TOXIPROXY.find_and_reset_proxy("socket");
            assert!(result.is_ok());
            assert!(!result.as_ref().unwrap().proxy_pack.enabled);
        })
        .is_ok());

    let result = TOXIPROXY.find_and_reset_proxy("socket");
    assert!(result.is_ok());
    assert!(result.as_ref().unwrap().proxy_pack.enabled);
}

#[test]
fn test_proxy_apply_with_latency() {
    populate_example();

    let proxy_result = TOXIPROXY.find_and_reset_proxy("socket");
    assert!(proxy_result.is_ok());

    let proxy_toxics = proxy_result.as_ref().unwrap().toxics();
    assert!(proxy_toxics.is_ok());
    assert_eq!(0, proxy_toxics.as_ref().unwrap().len());

    let apply_result = proxy_result
        .as_ref()
        .unwrap()
        .with_latency("downstream".into(), 2000, 0, 1.0)
        .apply(|| {
            let all = TOXIPROXY.all();
            assert!(all.is_ok());
            let proxy = all.as_ref().unwrap().get("socket");
            assert!(proxy.is_some());

            let proxy_toxics = proxy.as_ref().unwrap().toxics();
            assert!(proxy_toxics.is_ok());
            assert_eq!(1, proxy_toxics.as_ref().unwrap().len());
        });

    assert!(apply_result.is_ok());

    let proxy_toxics = proxy_result.as_ref().unwrap().toxics();
    assert!(proxy_toxics.is_ok());
    assert_eq!(0, proxy_toxics.as_ref().unwrap().len());
}

#[test]
fn test_proxy_apply_with_latency_as_separate_calls_for_test() {
    populate_example();

    let proxy_result = TOXIPROXY.find_and_reset_proxy("socket");
    assert!(proxy_result.is_ok());

    let proxy_toxics = proxy_result.as_ref().unwrap().toxics();
    assert!(proxy_toxics.is_ok());
    assert_eq!(0, proxy_toxics.as_ref().unwrap().len());

    let _ = proxy_result
        .as_ref()
        .unwrap()
        .with_latency("downstream".into(), 2000, 0, 1.0);

    let all = TOXIPROXY.all();
    assert!(all.is_ok());
    let proxy = all.as_ref().unwrap().get("socket");
    assert!(proxy.is_some());

    let proxy_toxics = proxy.as_ref().unwrap().toxics();
    assert!(proxy_toxics.is_ok());
    assert_eq!(1, proxy_toxics.as_ref().unwrap().len());
}

#[test]
fn test_proxy_apply_with_latency_with_real_request() {
    let server_thread = spawn(|| one_take_server());
    populate_example();

    let proxy_result = TOXIPROXY.find_and_reset_proxy("socket");
    assert!(proxy_result.is_ok());

    let apply_result = proxy_result
        .as_ref()
        .unwrap()
        .with_latency("downstream".into(), 2000, 0, 1.0)
        .apply(|| {
            let client_thread = spawn(|| one_shot_client());

            server_thread.join().expect("Failed closing server thread");
            let duration = client_thread.join().expect("Failed closing client thread");

            assert!(duration.as_secs() >= 2);
        });

    assert!(apply_result.is_ok());
}

#[test]
fn test_proxy_with_latency_with_two_real_http_requests() {
    populate_example();
    let proxy_result = TOXIPROXY.find_and_reset_proxy("socket");
    assert!(proxy_result.is_ok());

    proxy_result.as_ref().unwrap().with_latency_upon_condition(
        "upstream".into(),
        2000,
        0,
        1.0,
        Some(ToxicCondition::new_http_request_header_matcher(
            "x-api-key".into(),
            "123".into(),
        )),
    );

    // First roundtrip does not have latency.
    let server_thread = spawn(|| {
        let rt = tokio::runtime::Runtime::new().expect("failed to create local Runtime");
        rt.block_on(one_shot_http_server())
    });
    let client_thread = spawn(|| one_shot_http_client());

    server_thread.join().expect("Failed closing server thread");
    let duration = client_thread.join().expect("Failed closing client thread");
    assert!(duration.as_secs() < 2);

    println!("First roundtrip took {:?}", duration);

    // Second roundtrip has latency.
    let server_thread = spawn(|| {
        let rt = tokio::runtime::Runtime::new().expect("failed to create local Runtime");
        rt.block_on(one_shot_http_server())
    });
    let client_thread = spawn(|| one_shot_http_client());

    server_thread.join().expect("Failed closing server thread");
    let duration = client_thread.join().expect("Failed closing client thread");
    assert!(duration.as_secs() >= 2);
}

/**
 * Support functions.
 */

fn populate_example() {
    let result = TOXIPROXY.populate(vec![ProxyPack::new(
        "socket".into(),
        "localhost:2001".into(),
        "localhost:2000".into(),
    )]);

    assert!(result.is_ok());
}

fn one_shot_client() -> Duration {
    let t_start = SystemTime::now();

    let mut stream = TcpStream::connect("localhost:2001").expect("Failed to connect to server");

    stream
        .write("hello".as_bytes())
        .expect("Client failed sending request");

    stream
        .read(&mut [0u8; 1024])
        .expect("Client failed reading response");

    t_start.elapsed().expect("Cannot establish duration")
}

fn one_take_server() {
    let mut stream = TcpListener::bind("localhost:2000")
        .expect("TcpListener cannot connect")
        .incoming()
        .next()
        .expect("Failed to listen for incoming")
        .expect("Request failes");

    stream
        .read(&mut [0u8; 1024])
        .expect("Server failed reading request");

    stream
        .write("byebye".as_bytes())
        .expect("Server failed writing response");

    stream.flush().expect("Failed flushing connection");
}

fn one_shot_http_client() -> Duration {
    let t_start = SystemTime::now();
    let client = reqwest::blocking::Client::builder().build().unwrap();
    let resp = client
        .get("http://localhost:2001/example")
        .header("x-api-key", "123")
        .send()
        .expect("Failed sending request");
    assert!(resp.status().is_success());
    t_start.elapsed().expect("Cannot establish duration")
}

async fn one_shot_http_server() {
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let arc_tx = Arc::new(Mutex::new(Some(tx)));

    // build our application with a single route that sends signal to shut down server after serving one request.
    let app = Router::new().route(
        "/example",
        get(|| async move {
            // Send signal to shut down server.
            if let Some(tx) = arc_tx.lock().await.take() {
                let _ = tx.send(());
            }

            "Hello, World!"
        }),
    );

    // run it with hyper on localhost:2000
    axum::Server::bind(&"0.0.0.0:2000".parse().unwrap())
        .serve(app.into_make_service())
        .with_graceful_shutdown(async {
            rx.await.ok();
        })
        .await
        .unwrap();

    println!("Server has shut down");
}
