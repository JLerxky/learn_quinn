#[macro_use]
extern crate tracing as logger;

use anyhow::{Context, Result};

use logger::Level;
use quinn::Connection;
use tracing_subscriber::{
    fmt::{format::Writer, time::FormatTime},
    EnvFilter,
};

pub fn set_log(filter: &str) {
    struct LocalTimer;
    impl FormatTime for LocalTimer {
        fn format_time(&self, w: &mut Writer<'_>) -> std::fmt::Result {
            write!(w, "{}", chrono::Local::now().format("%m-%d %T%.3f"))
        }
    }
    let filter = EnvFilter::try_new(filter).unwrap();
    // tracing 初始化
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_timer(LocalTimer)
        .with_thread_ids(true)
        .with_env_filter(filter)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
}

pub fn read_cert_from_file(
    cert_path: &str,
    key_path: &str,
) -> Result<(rustls::Certificate, rustls::PrivateKey)> {
    // Read from certificate and key from directory.
    let (cert, key) = std::fs::read(cert_path).and_then(|x| Ok((x, std::fs::read(key_path)?)))?;

    // Parse to certificate chain whereafter taking the first certifcater in this chain.
    let cert = rustls::Certificate(cert);
    let key = rustls::PrivateKey(key);

    Ok((cert, key))
}

pub fn generate_self_signed_cert(
    cert_path: &str,
    key_path: &str,
) -> Result<(rustls::Certificate, rustls::PrivateKey)> {
    if let Ok((cert, key)) = read_cert_from_file(cert_path, key_path) {
        info!("The certificate already exists");
        return Ok((cert, key));
    }

    // Generate dummy certificate.
    let certificate = rcgen::generate_simple_self_signed(vec!["localhost".into()])?;
    let serialized_key = certificate.serialize_private_key_der();
    let serialized_certificate = certificate.serialize_der()?;

    // Write to files.
    std::fs::write(cert_path, &serialized_certificate).context("failed to write certificate")?;
    std::fs::write(key_path, &serialized_key).context("failed to write private key")?;

    info!("The certificate was generated successfully");

    let cert = rustls::Certificate(serialized_certificate);
    let key = rustls::PrivateKey(serialized_key);
    Ok((cert, key))
}

pub async fn send_bi(conn: Connection, buf: &[u8]) -> Result<Vec<u8>> {
    let (mut send, mut recv) = conn.open_bi().await?;
    let _ = send.write_all(buf).await;
    let _ = send.finish().await;

    Ok(recv.read_to_end(1024).await?)
}

pub async fn send_uni(conn: Connection, buf: &[u8]) -> Result<()> {
    let mut send = conn.open_uni().await?;
    let _ = send.write_all(buf).await;
    let _ = send.finish().await;
    Ok(())
}

pub async fn send_dg(conn: Connection, data: &[u8]) -> Result<()> {
    Ok(conn.send_datagram(data.to_vec().into())?)
}
