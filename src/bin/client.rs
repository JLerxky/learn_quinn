use futures_util::StreamExt;
use learn_quinn::send_bi;

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
            connection: conn,
            bi_streams,
            ..
        } = new_conn;

        // 主动发送端
        tokio::spawn(async move {
            let mut i = 0;
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs_f64(1f64));
            loop {
                interval.tick().await;
                if let Ok(resp) = send_bi(conn.clone(), i.to_string().as_bytes()).await {
                    println!("resp: {}", String::from_utf8_lossy(resp.as_slice()));
                }
                i += 1;
            }
        });

        // 接收端
        client_handle_bi(bi_streams).await;

        endpoint.wait_idle().await;
    }
}

async fn client_handle_bi(mut bi_streams: quinn::IncomingBiStreams) {
    while let Some(stream) = bi_streams.next().await {
        let stream = match stream {
            Err(quinn::ConnectionError::ApplicationClosed { .. }) => {
                println!("connection closed");
                return;
            }
            Err(e) => {
                println!("connection error: {}", e);
                return;
            }
            Ok(s) => s,
        };
        let (mut _send, recv) = stream;
        let resp = recv.read_to_end(usize::max_value()).await.unwrap();
        println!("resp: {}", String::from_utf8_lossy(resp.as_slice()));
    }
}
