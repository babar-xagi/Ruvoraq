use std::{net::SocketAddr, sync::Arc, time::Duration};

use ruvoraq_web::{App, Settings};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{Notify, oneshot},
    time::timeout,
};

const DEADLINE: Duration = Duration::from_secs(5);

async fn request(address: SocketAddr, method: &str, path: &str) -> String {
    timeout(DEADLINE, async {
        let mut stream = TcpStream::connect(address).await.unwrap();
        stream
            .write_all(format!("{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Length: 0\r\n\r\n").as_bytes())
            .await
            .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).await.unwrap();
        response
    })
    .await
    .expect("HTTP request completed before its deadline")
}

#[tokio::test]
async fn real_http_routes_methods_responses_and_fallbacks() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (stop, stopped) = oneshot::channel();
    let app = App::new()
        .get("/items", || async { "read" })
        .post("/items", || async { "create" })
        .put("/items", || async { "replace" })
        .patch("/items", || async { "update" })
        .delete("/items", || async { "delete" });
    let server = tokio::spawn(app.serve(listener, async { stopped.await.unwrap() }));

    for (method, body) in [
        ("GET", "read"),
        ("POST", "create"),
        ("PUT", "replace"),
        ("PATCH", "update"),
        ("DELETE", "delete"),
    ] {
        let response = request(address, method, "/items").await;
        assert!(response.starts_with("HTTP/1.1 200 OK\r\n"), "{response}");
        assert!(
            response.contains("content-type: text/plain; charset=utf-8"),
            "{response}"
        );
        assert_eq!(response.split("\r\n\r\n").nth(1).unwrap(), body);
    }
    let response = request(address, "HEAD", "/items").await;
    assert!(response.starts_with("HTTP/1.1 200"));
    assert!(response.ends_with("\r\n\r\n"));
    assert!(
        request(address, "GET", "/missing")
            .await
            .starts_with("HTTP/1.1 404")
    );
    assert!(
        request(address, "OPTIONS", "/items")
            .await
            .starts_with("HTTP/1.1 405")
    );

    stop.send(()).unwrap();
    timeout(DEADLINE, server).await.unwrap().unwrap().unwrap();
}

#[tokio::test]
async fn graceful_shutdown_finishes_an_in_flight_request() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let (stop, stopped) = oneshot::channel();
    let app = App::new().get("/slow", {
        let entered = entered.clone();
        let release = release.clone();
        move || {
            let entered = entered.clone();
            let release = release.clone();
            async move {
                entered.notify_one();
                release.notified().await;
                "finished"
            }
        }
    });
    let server = tokio::spawn(app.serve(listener, async { stopped.await.unwrap() }));
    let client = tokio::spawn(async move { request(address, "GET", "/slow").await });
    timeout(DEADLINE, entered.notified()).await.unwrap();
    stop.send(()).unwrap();
    tokio::task::yield_now().await;
    assert!(
        !server.is_finished(),
        "shutdown must wait for the active request"
    );
    release.notify_one();
    let response = timeout(DEADLINE, client).await.unwrap().unwrap();
    assert!(response.ends_with("finished"), "{response}");
    timeout(DEADLINE, server).await.unwrap().unwrap().unwrap();
    assert!(TcpStream::connect(address).await.is_err());
}

#[tokio::test]
async fn occupied_address_returns_a_clear_bind_error() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let error = App::new()
        .settings(Settings {
            address,
            ..Settings::default()
        })
        .run()
        .await
        .unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::AddrInUse);
    assert!(error.to_string().contains("cannot bind server"));
    assert!(error.to_string().contains(&address.to_string()));
}
