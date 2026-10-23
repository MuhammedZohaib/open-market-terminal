use omt_providers::Http;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
async fn server(response: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut b = [0; 4096];
        let read = stream.read(&mut b).await.unwrap();
        assert!(read > 0);
        stream.write_all(response.as_bytes()).await.unwrap();
    });
    format!("http://{address}/test")
}
#[tokio::test]
async fn http_401_is_actionable() {
    let url =
        server("HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await;
    let e = Http::new().unwrap().json(&url).await.unwrap_err();
    assert!(e.to_string().contains("401"));
}
#[tokio::test]
async fn malformed_json_is_error() {
    let url = server("HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\nnope").await;
    assert!(Http::new().unwrap().json(&url).await.is_err());
}
#[tokio::test]
async fn retry_after_is_enforced() {
    let url=server("HTTP/1.1 429 Too Many Requests\r\nRetry-After: 2\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await;
    let http = Http::new().unwrap();
    assert!(
        http.json(&url)
            .await
            .unwrap_err()
            .to_string()
            .contains("2s")
    );
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(30), http.json(&url))
            .await
            .is_err()
    );
}
#[tokio::test]
async fn credentials_are_not_in_network_errors() {
    let url = server(
        "HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
    )
    .await;
    let e = Http::new()
        .unwrap()
        .json(&format!("{url}?api_key=DO_NOT_LOG_THIS"))
        .await
        .unwrap_err();
    assert!(!e.to_string().contains("DO_NOT_LOG_THIS"));
}
#[tokio::test]
async fn connection_failure_does_not_panic() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    assert!(
        Http::new()
            .unwrap()
            .json(&format!("http://{address}"))
            .await
            .is_err()
    );
}
