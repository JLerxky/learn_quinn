use std::ascii;

use anyhow::{anyhow, Result};
use futures_util::{StreamExt, TryFutureExt};
use learn_quinn::generate_self_signed_cert;
fn main() {
    let quinn_runtime = tokio::runtime::Runtime::new().unwrap();
    quinn_runtime.block_on(server("./cert/cert.pem", "./cert/privkey.pem"));
}
async fn server(cert_path: &str, key_path: &str) {
    if let Ok((cert_chain, key)) = generate_self_signed_cert(cert_path, key_path) {
        if let Ok(server_config) = quinn::ServerConfig::with_single_cert(vec![cert_chain], key) {
            if let Ok((endpoint, mut incoming)) = quinn::Endpoint::server(
                server_config,
                "127.0.0.1:5001".parse::<std::net::SocketAddr>().unwrap(),
            ) {
                println!("listening on {}", endpoint.local_addr().unwrap());
                while let Some(conn) = incoming.next().await {
                    println!("connection incoming");
                    tokio::spawn(handle_connection(conn).unwrap_or_else(move |e| {
                        println!("connection failed: {reason}", reason = e.to_string())
                    }));
                }
            }
        }
    }
}

async fn handle_connection(conn: quinn::Connecting) -> Result<()> {
    let quinn::NewConnection {
        connection,
        mut bi_streams,
        ..
    } = conn.await?;
    async {
        println!("remote: {}", connection.remote_address());

        // Each stream initiated by the client constitutes a new request.
        while let Some(stream) = bi_streams.next().await {
            let stream = match stream {
                Err(quinn::ConnectionError::ApplicationClosed { .. }) => {
                    println!("connection closed");
                    return Ok(());
                }
                Err(e) => {
                    return Err(e);
                }
                Ok(s) => s,
            };
            tokio::spawn(
                handle_request(stream)
                    .unwrap_or_else(move |e| println!("failed: {reason}", reason = e.to_string())),
            );
        }
        Ok(())
    }
    .await?;
    Ok(())
}

async fn handle_request((mut send, recv): (quinn::SendStream, quinn::RecvStream)) -> Result<()> {
    let req = recv
        .read_to_end(64 * 1024)
        .await
        .map_err(|e| anyhow!("failed reading request: {}", e))?;
    let mut escaped = String::new();
    for &x in &req[..] {
        let part = ascii::escape_default(x).collect::<Vec<_>>();
        escaped.push_str(std::str::from_utf8(&part).unwrap());
    }
    // Execute the request
    println!("context: {}", &escaped);
    // Write the response
    send.write_all(&req)
        .await
        .map_err(|e| anyhow!("failed to send response: {}", e))?;
    // Gracefully terminate the stream
    send.finish()
        .await
        .map_err(|e| anyhow!("failed to shutdown stream: {}", e))?;
    println!("complete");
    Ok(())
}
