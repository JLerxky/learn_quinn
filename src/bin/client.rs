fn main() {
    let quinn_runtime = tokio::runtime::Runtime::new().unwrap();
    quinn_runtime.block_on(client("./cert/cert.pem"));
}
async fn client(cert_path: &str) {
    let cert = std::fs::read(&cert_path).unwrap();
    let mut roots = rustls::RootCertStore::empty();
    roots.add(&rustls::Certificate(cert)).unwrap();

    if let Ok(mut endpoint) =
        quinn::Endpoint::client("[::]:0".parse::<std::net::SocketAddr>().unwrap())
    {
        endpoint.set_default_client_config(quinn::ClientConfig::with_root_certificates(roots));

        let new_conn = endpoint
            .connect(
                "127.0.0.1:5001".parse::<std::net::SocketAddr>().unwrap(),
                "localhost",
            )
            .unwrap()
            .await
            .unwrap();
        println!("connected success");

        let quinn::NewConnection {
            connection: conn, ..
        } = new_conn;

        if let Ok((mut send, recv)) = conn.open_bi().await {
            let _ = send.write_all(b"hello").await;
            let _ = send.finish().await;

            let resp = recv.read_to_end(usize::max_value()).await.unwrap();
            println!("resp: {}", String::from_utf8_lossy(resp.as_slice()));
        }

        conn.close(0u32.into(), b"done");
        endpoint.wait_idle().await;
    }
}
