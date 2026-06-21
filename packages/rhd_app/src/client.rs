use std::path::Path;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

use crate::ipc::protocol::{IpcRequest, IpcResponse};

const SOCKET_PATH: &str = "rhd.sock";

pub async fn run_scenario(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let socket_path = Path::new(SOCKET_PATH);
    let mut stream = UnixStream::connect(socket_path).await.map_err(|e| {
        format!(
            "failed to connect to daemon at {}: {} (is daemon running?)",
            SOCKET_PATH, e
        )
    })?;

    let request = IpcRequest::RunScenario {
        name: name.to_string(),
    };
    let request_bytes = rkyv::to_bytes::<_, 256>(&request)
        .map_err(|e| format!("failed to serialize request: {}", e))?;

    let payload_len = request_bytes.len() as u32;
    stream.write_all(&crate::ipc::protocol::PROTOCOL_VERSION.to_be_bytes()).await?;
    stream.write_all(&payload_len.to_be_bytes()).await?;
    stream.write_all(&request_bytes).await?;
    stream.flush().await?;

    let mut version_bytes = [0u8; 4];
    stream.read_exact(&mut version_bytes).await?;
    let version = u32::from_be_bytes(version_bytes);
    if version != crate::ipc::protocol::PROTOCOL_VERSION {
        return Err(format!("unsupported protocol version from daemon: {}", version).into());
    }

    let mut length_bytes = [0u8; 4];
    stream.read_exact(&mut length_bytes).await?;
    let response_len = u32::from_be_bytes(length_bytes) as usize;

    let mut response_buf = vec![0u8; response_len];
    stream.read_exact(&mut response_buf).await?;

    use rkyv::validation::validators::DefaultValidator;
    use rkyv::{Archive, CheckBytes, Deserialize};
    let archived = rkyv::check_archived_root::<IpcResponse>(&response_buf)
        .map_err(|e| format!("invalid response from daemon: {}", e))?;
    let response: IpcResponse = archived
        .deserialize(&mut rkyv::de::deserializers::SharedDeserializeMap::new())
        .map_err(|e| format!("failed to deserialize response: {}", e))?;

    match response {
        IpcResponse::Success { output } => {
            print!("{}", output);
            Ok(())
        }
        IpcResponse::Error { message } => Err(message.into()),
    }
}
