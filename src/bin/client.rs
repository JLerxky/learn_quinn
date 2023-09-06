#[macro_use]
extern crate tracing as logger;

use std::{sync::Arc, time::Duration};

use anyhow::Result;
use learn_quinn::set_log;
use quinn::{Connection, ConnectionError};

fn main() -> Result<()> {
    set_log("info");
    let quinn_runtime = tokio::runtime::Runtime::new()?;
    // for _ in 0..9999 {
    //     quinn_runtime.spawn(client("./cert/cert.pem"));
    // }
    quinn_runtime.block_on(client("./cert/cert.pem"))?;
    Ok(())
}

async fn client(cert_path: &str) -> Result<()> {
    let cert = std::fs::read(cert_path)?;
    let mut roots = rustls::RootCertStore::empty();
    roots.add(&rustls::Certificate(cert))?;

    let mut endpoint = quinn::Endpoint::client("[::]:0".parse::<std::net::SocketAddr>()?)?;
    let mut client_config = quinn::ClientConfig::with_root_certificates(roots);
    let mut transport_config = quinn::TransportConfig::default();
    transport_config.keep_alive_interval(Some(Duration::from_secs(5)));
    client_config.transport_config(Arc::new(transport_config));
    endpoint.set_default_client_config(client_config);

    let new_conn = endpoint
        .connect(
            "127.0.0.1:2101".parse::<std::net::SocketAddr>()?,
            "localhost",
        )?
        .await?;
    info!("connected success");

    // 接收端
    tokio::select! {
        _ = client_read_datagrams(&new_conn) => {},
        _ = client_read_uni(&new_conn) => {},
        _ = client_read_bi(&new_conn) => {},
        _ = client_send_uni(&new_conn) => {},
    }

    endpoint.wait_idle().await;
    Ok(())
}

async fn client_read_bi(connection: &Connection) {
    loop {
        match connection.accept_bi().await {
            Err(ConnectionError::ApplicationClosed { .. }) => {
                info!("connection closed");
                break;
            }
            Err(e) => {
                info!("connection error: {}", e);
                break;
            }
            Ok(stream) => {
                tokio::spawn(client_handle_stream(Some(stream.0), stream.1));
            }
        }
    }
}

async fn client_read_uni(connection: &Connection) {
    loop {
        match connection.accept_uni().await {
            Err(ConnectionError::ApplicationClosed { .. }) => {
                info!("connection closed");
                break;
            }
            Err(e) => {
                info!("connection error: {}", e);
                break;
            }
            Ok(stream) => {
                tokio::spawn(client_handle_stream(None, stream));
            }
        }
    }
}

async fn client_read_datagrams(connection: &Connection) {
    loop {
        match connection.read_datagram().await {
            Err(ConnectionError::ApplicationClosed { .. }) => {
                info!("connection closed");
                break;
            }
            Err(e) => {
                info!("connection error: {}", e);
                break;
            }
            Ok(datagram) => {
                info!("resp: {}", String::from_utf8_lossy(&datagram));
            }
        };
    }
}

pub async fn client_send_uni(conn: &Connection) -> Result<()> {
    let mut send_i = tokio::time::interval(Duration::from_secs(1));
    loop {
        send_i.tick().await;
        let mut send = conn.open_uni().await.map_err(|e| {
            error!("{e}");
            e
        })?;
        let _ = send.write_all(b"hello").await;
        let _ = send.finish().await;
    }
}

pub async fn client_handle_stream(
    _send: Option<quinn::SendStream>,
    mut recv: quinn::RecvStream,
) -> Result<()> {
    let resp = recv.read_to_end(usize::max_value()).await?;
    info!("resp: {}", String::from_utf8_lossy(resp.as_slice()));
    Ok(())
}
