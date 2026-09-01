use async_trait::async_trait;
use libp2p::request_response::Codec;
use std::io;
use futures::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use crate::network::protocol::P2PMessage;

#[derive(Clone, Default)]
pub struct QuantaCodec;

#[async_trait]
impl Codec for QuantaCodec {
    type Protocol = &'static str;
    type Request = P2PMessage;
    type Response = P2PMessage;

    async fn read_request<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
    ) -> io::Result<Self::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        let mut len_buf = [0u8; 4];
        io.read_exact(&mut len_buf).await?;
        let len = u32::from_be_bytes(len_buf) as usize;
        if len > 8 * 1024 * 1024 { return Err(io::Error::new(io::ErrorKind::InvalidData, "Too large")); }
        let mut vec = vec![0u8; len];
        io.read_exact(&mut vec).await?;
        bincode::deserialize(&vec).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    async fn read_response<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
    ) -> io::Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        let mut len_buf = [0u8; 4];
        io.read_exact(&mut len_buf).await?;
        let len = u32::from_be_bytes(len_buf) as usize;
        if len > 8 * 1024 * 1024 { return Err(io::Error::new(io::ErrorKind::InvalidData, "Too large")); }
        let mut vec = vec![0u8; len];
        io.read_exact(&mut vec).await?;
        bincode::deserialize(&vec).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    async fn write_request<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
        req: Self::Request,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        let data = bincode::serialize(&req).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        io.write_all(&(data.len() as u32).to_be_bytes()).await?;
        io.write_all(&data).await?;
        io.close().await
    }

    async fn write_response<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
        res: Self::Response,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        let data = bincode::serialize(&res).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        io.write_all(&(data.len() as u32).to_be_bytes()).await?;
        io.write_all(&data).await?;
        io.close().await
    }
}
