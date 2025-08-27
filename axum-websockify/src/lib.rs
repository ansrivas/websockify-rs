use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::ConnectInfo,
    routing::get,
    Router,
};
use std::{
    io,
    net::{SocketAddr, ToSocketAddrs},
    sync::Arc,
};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tracing::{debug, error, info};
pub mod error;
pub mod utils;
use futures_util::sink::SinkExt;
pub use tokio_wireguard::{Interface as WgInterface, TcpStream as WgTcpStream};

pub enum Destination {
    Tcp(Vec<SocketAddr>),
    WireGuard {
        addrs: Vec<SocketAddr>,
        interface: Arc<WgInterface>,
    },
}

impl std::fmt::Display for Destination {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        match self {
            Destination::Tcp(tcp) => write!(f, "{:?}", tcp),
            Destination::WireGuard { addrs, interface } => write!(f, "{:?} {:?}", addrs, interface),
        }
    }
}

impl Destination {
    pub fn tcp(addr: impl ToSocketAddrs) -> io::Result<Destination> {
        Ok(Destination::Tcp(addr.to_socket_addrs()?.collect()))
    }

    pub fn wireguard(
        addrs: impl ToSocketAddrs,
        interface: Arc<WgInterface>,
    ) -> io::Result<Destination> {
        Ok(Destination::WireGuard {
            addrs: addrs.to_socket_addrs()?.collect(),
            interface,
        })
    }

    async fn connect(&self) -> io::Result<NetStream> {
        match self {
            Destination::Tcp(addrs) => {
                let mut last_error = None;
                for one in addrs {
                    match TcpStream::connect(one).await {
                        Ok(stream) => return Ok(NetStream::Tcp(stream)),
                        Err(e) => last_error = Some(e),
                    }
                }
                Err(last_error.unwrap())
            }
            Destination::WireGuard { addrs, interface } => {
                let mut last_error = None;
                for addr in addrs {
                    match WgTcpStream::connect(*addr, interface.as_ref()).await {
                        Ok(stream) => return Ok(NetStream::WireGuard(stream)),
                        Err(e) => last_error = Some(e),
                    }
                }
                Err(last_error.unwrap_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::AddrNotAvailable,
                        "No addresses to connect to",
                    )
                }))
            }
        }
    }
}

enum NetStream {
    Tcp(TcpStream),
    WireGuard(WgTcpStream),
}

pub fn create_router(dest: Destination) -> Router {
    let dest = Arc::new(dest);
    Router::new().route(
        "/",
        get(
            move |ws: WebSocketUpgrade, ConnectInfo(addr): ConnectInfo<SocketAddr>| {
                let dest = dest.clone();
                async move { ws.on_upgrade(move |socket| handle_socket(socket, addr, dest)) }
            },
        ),
    )
}

async fn handle_socket(ws: WebSocket, addr: SocketAddr, dest: Arc<Destination>) {
    match dest.connect().await {
        Ok(stream) => {
            info!("{} target:[{}] Connection started", addr, dest.as_ref());
            if let Err(e) = match stream {
                NetStream::Tcp(x) => handle_connection(addr, ws, x).await,
                NetStream::WireGuard(x) => handle_connection(addr, ws, x).await,
            } {
                error!("{}: Error: {}", addr, e);
            }
        }
        Err(e) => {
            error!("{} target:[{}] {}", addr, dest.as_ref(), e);
        }
    }
}

async fn handle_connection<S>(
    addr: SocketAddr,
    mut ws: WebSocket,
    mut stream: S,
) -> Result<(), Box<dyn std::error::Error>>
where
    S: AsyncRead + AsyncWrite + std::marker::Unpin,
{
    let mut buffer = vec![0; 16384];

    loop {
        tokio::select! {
            message = ws.recv() => {
                match message {
                    Some(Ok(message)) => {
                        match message{
                            Message::Binary(data) => {
                                stream.write_all(&data).await?;
                            }
                            Message::Text(data) => {
                                stream.write_all(&data.as_bytes()[..]).await?;
                            }
                            Message::Ping(data) => {
                                stream.write_all(&data).await?;

                            }
                            Message::Pong(data) => {
                                stream.write_all(&data).await?;

                            }
                            Message::Close(data) => {
                                debug!("{}: Web socket closed {:?}", addr, data);
                                return Ok(());
                            }
                        }


                    }
                    None => {
                        error!("{}: No packet received from websocket", addr);
                        break;
                    }
                    Some(Err(e)) => return Err(e.into()),
                }
            },
            result = stream.read(&mut buffer) => {
                match result {
                    Ok(n) if n > 0 => {
                        ws.send(Message::Binary(buffer[..n].to_vec().into())).await?;
                    }
                    Ok(_) => {
                        debug!("{}: TCP/Unix stream closed", addr);
                        ws.close().await?;
                        return Ok(());
                    }
                    Err(e) => return Err(e.into()),
                }
            }
        }
    }
    Ok(())
}
