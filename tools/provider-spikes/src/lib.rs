//! Development-only native SDK feasibility probe; no CLI or credentials.
#[cfg(test)]
mod tests {
    use github_copilot_sdk::Client;
    use serde_json::{Value, json};
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

    #[tokio::test]
    async fn copilot_native_rpc_and_shutdown_over_mock_streams() {
        let (client_stream, server_stream) = tokio::io::duplex(8192);
        let (reader, writer) = tokio::io::split(client_stream);
        let client =
            Client::from_streams(reader, writer, std::env::current_dir().unwrap()).unwrap();
        let server = tokio::spawn(async move {
            let (reader, mut writer) = tokio::io::split(server_stream);
            let mut reader = BufReader::new(reader);
            let mut length = 0;
            loop {
                let mut line = String::new();
                assert_ne!(reader.read_line(&mut line).await.unwrap(), 0);
                if line == "\r\n" {
                    break;
                }
                if let Some(value) = line.strip_prefix("Content-Length: ") {
                    length = value.trim().parse::<usize>().unwrap();
                }
            }
            assert!(length > 0 && length < 8192);
            let mut body = vec![0; length];
            reader.read_exact(&mut body).await.unwrap();
            let request: Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(request["method"], "ping");
            assert_eq!(request["params"]["message"], "rust-spike");
            let response = serde_json::to_vec(&json!({"jsonrpc":"2.0","id":request["id"],"result":{"message":"rust-spike","timestamp":"2026-10-27T00:00:00Z","protocolVersion":3}})).unwrap();
            writer
                .write_all(format!("Content-Length: {}\r\n\r\n", response.len()).as_bytes())
                .await
                .unwrap();
            writer.write_all(&response).await.unwrap();
            writer.flush().await.unwrap();
            // EOF demonstrates client.stop closes the supplied transport.
            let mut remaining = Vec::new();
            reader.read_to_end(&mut remaining).await.unwrap();
        });
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            client.ping(Some("rust-spike")),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(result.message, "rust-spike");
        assert_eq!(result.protocol_version, Some(3));
        match tokio::time::timeout(std::time::Duration::from_secs(5), client.stop()).await {
            Ok(Ok(())) => {}
            outcome => {
                client.force_stop();
                panic!("SDK stop failed: {outcome:?}");
            }
        }
        tokio::time::timeout(std::time::Duration::from_secs(5), server)
            .await
            .unwrap()
            .unwrap();
    }
}
