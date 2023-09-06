#[macro_use]
extern crate tracing as logger;

use std::{ascii, sync::Arc, time::Duration};

use anyhow::{anyhow, Result};
use learn_quinn::{generate_self_signed_cert, send_bi, send_dg, send_uni, set_log};
use quinn::{Connection, ConnectionError};

fn main() -> Result<()> {
    set_log("info");
    let quinn_runtime = tokio::runtime::Runtime::new()?;
    quinn_runtime.block_on(Server::start("./cert/cert.pem", "./cert/privkey.pem"))?;
    Ok(())
}

pub struct Server {}

impl Server {
    pub async fn start(cert_path: &str, key_path: &str) -> Result<()> {
        let (cert_chain, key) = generate_self_signed_cert(cert_path, key_path)?;
        let mut server_config = quinn::ServerConfig::with_single_cert(vec![cert_chain], key)?;
        let mut transport_config = quinn::TransportConfig::default();
        transport_config.keep_alive_interval(Some(Duration::from_secs(5)));
        server_config.transport_config(Arc::new(transport_config));
        let endpoint = quinn::Endpoint::server(
            server_config,
            "127.0.0.1:2101".parse::<std::net::SocketAddr>()?,
        )?;
        info!("listening on {}", endpoint.local_addr()?);
        while let Some(connecting) = endpoint.accept().await {
            info!("connection incoming");
            tokio::spawn(server_handle_connection(connecting));
        }
        Ok(())
    }
}

async fn server_handle_connection(connecting: quinn::Connecting) -> Result<()> {
    let connection = connecting.await?;
    info!("remote: {}", &connection.remote_address());

    tokio::spawn(send_bi(connection.clone(), b"bi: welcome"));

    tokio::spawn(send_uni(connection.clone(), b"uni: welcome"));

    tokio::spawn(send_dg(connection.clone(), b"bg: welcome"));

    tokio::select! {
        _ = server_read_datagrams(&connection) => {},
        _ = server_read_uni(&connection) => {},
        _ = server_read_bi(&connection) => {},
    }
    Ok(())
}

async fn server_read_bi(connection: &Connection) {
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
                tokio::spawn(server_handle_stream(Some(stream.0), stream.1));
            }
        }
    }
}

async fn server_read_uni(connection: &Connection) {
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
                tokio::spawn(server_handle_stream(None, stream));
            }
        };
    }
}

async fn server_read_datagrams(connection: &Connection) {
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

async fn server_handle_stream(
    send: Option<quinn::SendStream>,
    mut recv: quinn::RecvStream,
) -> Result<()> {
    let req = recv
        .read_to_end(64 * 1024)
        .await
        .map_err(|e| anyhow!("failed reading request: {e}"))?;

    let mut escaped = String::new();
    for &x in &req[..] {
        let part = ascii::escape_default(x).collect::<Vec<_>>();
        escaped.push_str(std::str::from_utf8(&part)?);
    }
    info!("context: {}", &escaped);

    if let Some(mut send) = send {
        send.write_all(&req)
            .await
            .map_err(|e| anyhow!("failed to send response: {e}"))?;
        send.finish()
            .await
            .map_err(|e| anyhow!("failed to shutdown stream: {e}"))?;
    }

    Ok(())
}
