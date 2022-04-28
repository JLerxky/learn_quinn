use std::{sync::Arc, time::Duration};

use anyhow::Result;
use futures_util::{StreamExt, TryFutureExt};

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
        let mut client_config = quinn::ClientConfig::with_root_certificates(roots);
        Arc::get_mut(&mut client_config.transport)
            .unwrap()
            .keep_alive_interval(Some(Duration::from_secs(5)));
        endpoint.set_default_client_config(client_config);

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
            bi_streams,
            uni_streams,
            datagrams,
            ..
        } = new_conn;

        // 接收端
        tokio::select! {
            _ = client_read_datagrams(datagrams) => {},
            _ = client_read_uni(uni_streams) => {},
            _ = client_read_bi(bi_streams) => {},
        }

        endpoint.wait_idle().await;
    }
}

async fn client_read_bi(mut bi_streams: quinn::IncomingBiStreams) {
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
            client_handle_stream(None, recv).unwrap_or_else(move |e| println!("failed: {e}")),
        );
    }
}

pub async fn client_read_uni(mut uni_streams: quinn::IncomingUniStreams) {
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
            client_handle_stream(None, stream).unwrap_or_else(move |e| println!("failed: {e}")),
        );
    }
}

pub async fn client_read_datagrams(mut datagrams: quinn::Datagrams) {
    while let Some(datagram) = datagrams.next().await {
        let datagram = match datagram {
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
            client_handle_datagram(datagram).unwrap_or_else(move |e| println!("failed: {e}")),
        );
    }
}

pub async fn client_handle_stream(
    _send: Option<quinn::SendStream>,
    recv: quinn::RecvStream,
) -> Result<()> {
    let resp = recv.read_to_end(usize::max_value()).await.unwrap();
    println!("resp: {}", String::from_utf8_lossy(resp.as_slice()));
    Ok(())
}

pub async fn client_handle_datagram(datagram: bytes::Bytes) -> Result<()> {
    println!("resp: {}", String::from_utf8_lossy(&datagram));

    Ok(())
}
