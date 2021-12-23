use anyhow::Result;
use futures_util::{StreamExt, TryFutureExt};
use learn_quinn::send_uni;

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
                "127.0.0.1:2101".parse::<std::net::SocketAddr>().unwrap(),
                "localhost",
            )
            .unwrap()
            .await
            .unwrap();
        println!("connected success");

        let quinn::NewConnection {
            connection: conn,
            bi_streams,
            uni_streams,
            datagrams,
            ..
        } = new_conn;

        // 主动发送端
        tokio::spawn(async move {
            let mut i = 0;
            // let mut interval = tokio::time::interval(tokio::time::Duration::from_secs_f64(1f64));
            loop {
                // interval.tick().await;
                if let Err(_) = send_uni(conn.clone(), i.to_string().as_bytes()).await {
                    break;
                }
                i += 1;
            }
        });

        // 接收端
        tokio::select! {
            _ = client_handle_datagrams(datagrams) => {},
            _ = client_handle_uni(uni_streams) => {},
            _ = client_handle_bi(bi_streams) => {},
        }

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
        tokio::spawn(
            client_handle_request(None, recv)
                .unwrap_or_else(move |e| println!("failed: {reason}", reason = e.to_string())),
        );
    }
}

pub async fn client_handle_uni(mut uni_streams: quinn::IncomingUniStreams) {
    while let Some(stream) = uni_streams.next().await {
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
        tokio::spawn(
            client_handle_request(None, stream)
                .unwrap_or_else(move |e| println!("failed: {reason}", reason = e.to_string())),
        );
    }
}

pub async fn client_handle_datagrams(mut datagrams: quinn::Datagrams) {
    while let Some(stream) = datagrams.next().await {
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
        tokio::spawn(
            client_handle_bytes(stream)
                .unwrap_or_else(move |e| println!("failed: {reason}", reason = e.to_string())),
        );
    }
}

pub async fn client_handle_request(
    _send: Option<quinn::SendStream>,
    recv: quinn::RecvStream,
) -> Result<()> {
    let resp = recv.read_to_end(usize::max_value()).await.unwrap();
    println!("resp: {}", String::from_utf8_lossy(resp.as_slice()));
    Ok(())
}

pub async fn client_handle_bytes(bytes: bytes::Bytes) -> Result<()> {
    println!("resp: {}", String::from_utf8_lossy(&bytes));

    Ok(())
}
