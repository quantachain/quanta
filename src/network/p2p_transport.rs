use libp2p::{
    core::{
        upgrade::{InboundConnectionUpgrade, OutboundConnectionUpgrade, UpgradeInfo},
    },
    PeerId,
};
use futures::{future::BoxFuture, FutureExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use std::io;
use std::sync::Arc;
use futures_rustls::{TlsAcceptor, TlsConnector};
use rustls::pki_types::ServerName;

#[derive(Clone)]
pub struct QuantaAuth {
    pub node_id: String,
    pub server_config: Arc<rustls::ServerConfig>,
    pub client_config: Arc<rustls::ClientConfig>,
}

impl UpgradeInfo for QuantaAuth {
    type Info = &'static str;
    type InfoIter = std::iter::Once<Self::Info>;

    fn protocol_info(&self) -> Self::InfoIter {
        std::iter::once("/quanta/auth/1.0.0")
    }
}

pub enum TlsStream<C> {
    Server(futures_rustls::server::TlsStream<C>),
    Client(futures_rustls::client::TlsStream<C>),
}

impl<C: AsyncRead + AsyncWrite + Unpin> AsyncRead for TlsStream<C> {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> std::task::Poll<io::Result<usize>> {
        match self.get_mut() {
            TlsStream::Server(s) => std::pin::Pin::new(s).poll_read(cx, buf),
            TlsStream::Client(s) => std::pin::Pin::new(s).poll_read(cx, buf),
        }
    }
}

impl<C: AsyncRead + AsyncWrite + Unpin> AsyncWrite for TlsStream<C> {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<io::Result<usize>> {
        match self.get_mut() {
            TlsStream::Server(s) => std::pin::Pin::new(s).poll_write(cx, buf),
            TlsStream::Client(s) => std::pin::Pin::new(s).poll_write(cx, buf),
        }
    }
    fn poll_flush(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<io::Result<()>> {
        match self.get_mut() {
            TlsStream::Server(s) => std::pin::Pin::new(s).poll_flush(cx),
            TlsStream::Client(s) => std::pin::Pin::new(s).poll_flush(cx),
        }
    }
    fn poll_close(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<io::Result<()>> {
        match self.get_mut() {
            TlsStream::Server(s) => std::pin::Pin::new(s).poll_close(cx),
            TlsStream::Client(s) => std::pin::Pin::new(s).poll_close(cx),
        }
    }
}

impl<C> InboundConnectionUpgrade<C> for QuantaAuth
where
    C: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    type Output = (PeerId, TlsStream<C>);
    type Error = io::Error;
    type Future = BoxFuture<'static, Result<Self::Output, Self::Error>>;

    fn upgrade_inbound(self, socket: C, _: Self::Info) -> Self::Future {
        async move {
            let acceptor = TlsAcceptor::from(self.server_config);
            let mut tls_stream = acceptor.accept(socket).await?;
            
            let mut len_buf = [0u8; 4];
            tls_stream.read_exact(&mut len_buf).await?;
            let len = u32::from_be_bytes(len_buf) as usize;
            if len > 1024 { return Err(io::Error::new(io::ErrorKind::InvalidData, "ID too long")); }
            let mut id_buf = vec![0u8; len];
            tls_stream.read_exact(&mut id_buf).await?;
            let _peer_id_str = String::from_utf8(id_buf).map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid UTF8"))?;
            let peer_id = PeerId::random();
            
            let our_id = self.node_id.into_bytes();
            tls_stream.write_all(&(our_id.len() as u32).to_be_bytes()).await?;
            tls_stream.write_all(&our_id).await?;
            
            Ok((peer_id, TlsStream::Server(tls_stream)))
        }.boxed()
    }
}

impl<C> OutboundConnectionUpgrade<C> for QuantaAuth
where
    C: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    type Output = (PeerId, TlsStream<C>);
    type Error = io::Error;
    type Future = BoxFuture<'static, Result<Self::Output, Self::Error>>;

    fn upgrade_outbound(self, socket: C, _: Self::Info) -> Self::Future {
        async move {
            let connector = TlsConnector::from(self.client_config);
            let domain = ServerName::try_from("quanta.node").unwrap().to_owned();
            let mut tls_stream = connector.connect(domain, socket).await?;
            
            let our_id = self.node_id.into_bytes();
            tls_stream.write_all(&(our_id.len() as u32).to_be_bytes()).await?;
            tls_stream.write_all(&our_id).await?;
            
            let mut len_buf = [0u8; 4];
            tls_stream.read_exact(&mut len_buf).await?;
            let len = u32::from_be_bytes(len_buf) as usize;
            if len > 1024 { return Err(io::Error::new(io::ErrorKind::InvalidData, "ID too long")); }
            let mut id_buf = vec![0u8; len];
            tls_stream.read_exact(&mut id_buf).await?;
            let _peer_id_str = String::from_utf8(id_buf).map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid UTF8"))?;
            
            let peer_id = PeerId::random();
            Ok((peer_id, TlsStream::Client(tls_stream)))
        }.boxed()
    }
}
