use rkyv::validation::validators::DefaultValidator;
use rkyv::{Archive, CheckBytes, Deserialize, Serialize};
use std::io::{self, Read, Write};

pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
#[archive(check_bytes)]
pub enum IpcRequest {
    RunScenario { name: String, cwd: String },
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
#[archive(check_bytes)]
pub enum IpcResponse {
    Success { output: String },
    Error { message: String },
    Aborted,
}

pub fn write_message<T: rkyv::Serialize<rkyv::ser::serializers::AllocSerializer<256>>>(
    writer: &mut impl Write,
    message: &T,
) -> io::Result<()> {
    let archived = rkyv::to_bytes::<_, 256>(message)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
    let payload_len = archived.len() as u32;
    writer.write_all(&PROTOCOL_VERSION.to_be_bytes())?;
    writer.write_all(&payload_len.to_be_bytes())?;
    writer.write_all(&archived)?;
    writer.flush()?;
    Ok(())
}

pub fn read_message<T: Archive>(reader: &mut impl Read) -> io::Result<T>
where
    T::Archived: rkyv::Deserialize<T, rkyv::de::deserializers::SharedDeserializeMap>
        + for<'a> CheckBytes<DefaultValidator<'a>>,
{
    let mut version_bytes = [0u8; 4];
    reader.read_exact(&mut version_bytes)?;
    let version = u32::from_be_bytes(version_bytes);
    if version != PROTOCOL_VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unsupported protocol version: {}", version),
        ));
    }

    let mut length_bytes = [0u8; 4];
    reader.read_exact(&mut length_bytes)?;
    let payload_len = u32::from_be_bytes(length_bytes) as usize;

    let mut payload = vec![0u8; payload_len];
    reader.read_exact(&mut payload)?;

    let archived = rkyv::check_archived_root::<T>(&payload)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
    let deserialized: T = archived
        .deserialize(&mut rkyv::de::deserializers::SharedDeserializeMap::new())
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
    Ok(deserialized)
}
