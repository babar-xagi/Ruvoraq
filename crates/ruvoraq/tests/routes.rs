use std::{net::SocketAddr, time::Duration};

use ruvoraq::prelude::*;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
    time::timeout,
};

#[get("/")]
async fn hello() -> &'static str {
    "Hello"
}

#[post("/items")]
async fn create() -> &'static str {
    "created"
}

#[put("/items")]
async fn replace() -> &'static str {
    "replaced"
}

#[patch("/items")]
async fn update() -> &'static str {
    "updated"
}

#[delete("/items")]
async fn remove() -> &'static str {
    "deleted"
}

#[get("/shared")]
#[post("/shared")]
async fn shared() -> &'static str {
    "shared"
}

#[get("/disabled")]
#[cfg(any())]
async fn disabled() -> &'static str {
    "disabled"
}

#[cfg_attr(all(), cfg(any()))]
#[get("/also-disabled")]
async fn also_disabled() -> &'static str {
    "disabled"
}

mod nested {
    use ruvoraq::prelude::*;

    #[get("/nested")]
    async fn nested_route() -> &'static str {
        "nested"
    }
}

async fn request(address: SocketAddr, method: &str, path: &str) -> String {
    timeout(Duration::from_secs(5), async {
        let mut stream = TcpStream::connect(address).await.unwrap();
        stream.write_all(format!("{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Length: 0\r\n\r\n").as_bytes()).await.unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).await.unwrap();
        response
    }).await.unwrap()
}

#[tokio::test]
async fn attributed_routes_register_automatically_and_preserve_functions() {
    assert_eq!(hello().await, "Hello");
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (stop, stopped) = oneshot::channel();
    let app = App::auto().unwrap();
    let server = tokio::spawn(app.serve(listener, async { stopped.await.unwrap() }));

    for (method, path, body) in [
        ("GET", "/", "Hello"),
        ("POST", "/items", "created"),
        ("PUT", "/items", "replaced"),
        ("PATCH", "/items", "updated"),
        ("DELETE", "/items", "deleted"),
        ("GET", "/nested", "nested"),
        ("GET", "/shared", "shared"),
        ("POST", "/shared", "shared"),
    ] {
        let response = request(address, method, path).await;
        assert!(response.starts_with("HTTP/1.1 200 OK"), "{response}");
        assert_eq!(response.split("\r\n\r\n").nth(1).unwrap(), body);
    }
    for path in ["/disabled", "/also-disabled"] {
        assert!(
            request(address, "GET", path)
                .await
                .starts_with("HTTP/1.1 404")
        );
    }
    stop.send(()).unwrap();
    timeout(Duration::from_secs(5), server)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}
