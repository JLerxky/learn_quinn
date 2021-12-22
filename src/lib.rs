use anyhow::{Context, Result};

pub fn read_cert_from_file(
    cert_path: &str,
    key_path: &str,
) -> Result<(rustls::Certificate, rustls::PrivateKey)> {
    // Read from certificate and key from directory.
    let (cert, key) = std::fs::read(&cert_path).and_then(|x| Ok((x, std::fs::read(&key_path)?)))?;

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
        println!("The certificate already exists");
        return Ok((cert, key));
    }

    // Generate dummy certificate.
    let certificate = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let serialized_key = certificate.serialize_private_key_der();
    let serialized_certificate = certificate.serialize_der().unwrap();

    // Write to files.
    std::fs::write(&cert_path, &serialized_certificate).context("failed to write certificate")?;
    std::fs::write(&key_path, &serialized_key).context("failed to write private key")?;

    println!("The certificate was generated successfully");

    let cert = rustls::Certificate(serialized_certificate);
    let key = rustls::PrivateKey(serialized_key);
    Ok((cert, key))
}

pub async fn send_bi(conn: quinn::Connection, buf: &[u8]) -> Result<Vec<u8>> {
    let (mut send, recv) = conn.open_bi().await?;
    let _ = send.write_all(buf).await;
    let _ = send.finish().await;

    Ok(recv.read_to_end(1024).await?)
}

pub async fn send_uni(conn: quinn::Connection, buf: &[u8]) -> Result<()> {
    let mut send = conn.open_uni().await?;
    let _ = send.write_all(buf).await;
    let _ = send.finish().await;
    Ok(())
}

// pub async fn send_(conn: &quinn::Connection, data: bytes::bytes::Bytes) -> Result<()> {
//     Ok(conn.clone().send_datagram(data)?)
// }
