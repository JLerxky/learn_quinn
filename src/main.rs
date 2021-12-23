use anyhow::{anyhow, Result};
use futures_util::{StreamExt, TryFutureExt};
use learn_quinn::{generate_self_signed_cert, send_bi, send_uni};
use std::ascii;

fn main() {
    let quinn_runtime = tokio::runtime::Runtime::new().unwrap();
    quinn_runtime.block_on(NetServer::start("./cert/cert.pem", "./cert/privkey.pem"));
}

#[derive(Clone)]
pub struct NetServer {}

impl NetServer {
    pub async fn start(cert_path: &str, key_path: &str) {
        if let Ok((cert_chain, key)) = generate_self_signed_cert(cert_path, key_path) {
            if let Ok(server_config) = quinn::ServerConfig::with_single_cert(vec![cert_chain], key)
            {
                if let Ok((endpoint, mut incoming)) = quinn::Endpoint::server(
                    server_config,
                    "127.0.0.1:2101".parse::<std::net::SocketAddr>().unwrap(),
                ) {
                    println!("listening on {}", endpoint.local_addr().unwrap());
                    while let Some(conn) = incoming.next().await {
                        println!("connection incoming");
                        tokio::spawn(server_handle_connection(conn).unwrap_or_else(move |e| {
                            println!("connection failed: {reason}", reason = e.to_string())
                        }));
                    }
                }
            }
        }
    }
}

async fn server_handle_connection(conn: quinn::Connecting) -> Result<()> {
    let quinn::NewConnection {
        connection,
        bi_streams,
        uni_streams,
        datagrams,
        ..
    } = conn.await?;
    println!("remote: {}", &connection.remote_address());

    let connection1 = connection.clone();
    let connection2 = connection.clone();
    tokio::spawn(async move {
        if let Ok(resp) = send_bi(connection1, b"bi: welcome").await {
            println!("resp: {}", String::from_utf8_lossy(resp.as_slice()));
        }
    });

    tokio::spawn(async move {
        let _ = send_uni(connection, b"uni: welcome").await;
    });

    tokio::spawn(async move {
        let _ = connection2.send_datagram(bytes::Bytes::from("da: welcome"));
    });

    // Each stream initiated by the client constitutes a new request.
    tokio::select! {
        _ = server_handle_datagrams(datagrams) => {},
        _ = server_handle_uni(uni_streams) => {},
        _ = server_handle_bi(bi_streams) => {},
    }
    Ok(())
}

async fn server_handle_bi(mut bi_streams: quinn::IncomingBiStreams) {
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
        tokio::spawn(
            server_handle_request(Some(stream.0), stream.1)
                .unwrap_or_else(move |e| println!("failed: {reason}", reason = e.to_string())),
        );
    }
}

async fn server_handle_uni(mut uni_streams: quinn::IncomingUniStreams) {
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
            server_handle_request(None, stream)
                .unwrap_or_else(move |e| println!("failed: {reason}", reason = e.to_string())),
        );
    }
}

async fn server_handle_datagrams(mut datagrams: quinn::Datagrams) {
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
            server_handle_bytes(stream)
                .unwrap_or_else(move |e| println!("failed: {reason}", reason = e.to_string())),
        );
    }
}

async fn server_handle_request(
    send: Option<quinn::SendStream>,
    recv: quinn::RecvStream,
) -> Result<()> {
    let req = recv
        .read_to_end(64 * 1024)
        .await
        .map_err(|e| anyhow!("failed reading request: {}", e))?;

    let mut escaped = String::new();
    for &x in &req[..] {
        let part = ascii::escape_default(x).collect::<Vec<_>>();
        escaped.push_str(std::str::from_utf8(&part).unwrap());
    }
    println!("context: {}", &escaped);

    if let Some(mut send) = send {
        send.write_all(&req)
            .await
            .map_err(|e| anyhow!("failed to send response: {}", e))?;
        send.finish()
            .await
            .map_err(|e| anyhow!("failed to shutdown stream: {}", e))?;
    }

    Ok(())
}

async fn server_handle_bytes(bytes: bytes::Bytes) -> Result<()> {
    println!("resp: {}", String::from_utf8_lossy(&bytes));

    println!("complete");
    Ok(())
}
