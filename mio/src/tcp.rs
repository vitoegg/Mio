//! TCP: the obfuscation and the Shadowsocks handshake, then the relay of shadowsocks-rust.

use std::{io, net::SocketAddr, sync::Arc, time::Duration};

use log::{error, info, warn};
use shadowsocks::{
    ServerConfig,
    config::ServerType,
    context::{Context, SharedContext},
    net::{AcceptOpts, ConnectOpts, TcpListener, TcpSocketOpts, TcpStream as OutboundTcpStream, is_dual_stack_addr},
    relay::{
        socks5::Address,
        tcprelay::{ProxyServerStream, utils::copy_encrypted_bidirectional},
    },
};
use socket2::SockRef;
use tokio::{
    net::TcpStream,
    sync::{OwnedSemaphorePermit, Semaphore},
    time::{sleep, timeout},
};

use crate::obfs::{self, ObfsStream};

const DIAL_TIMEOUT: Duration = Duration::from_secs(10);
/// Covers the request head and the Shadowsocks handshake.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(30);
const KEEPALIVE: Duration = Duration::from_secs(15);
/// Unauthenticated connections beyond this are dropped, so they cannot exhaust memory or descriptors.
const MAX_HANDSHAKES: usize = 256;

fn socket_opts() -> TcpSocketOpts {
    TcpSocketOpts {
        nodelay: true,
        keepalive: Some(KEEPALIVE),
        ..Default::default()
    }
}

/// Accepts TCP Fast Open, so a client that supports it saves a round trip per connection.
pub async fn listen(addr: SocketAddr) -> io::Result<TcpListener> {
    let opts = AcceptOpts {
        tcp: TcpSocketOpts {
            fastopen: true,
            ..socket_opts()
        },
        ..Default::default()
    };
    let listener = TcpListener::bind_with_opts(&addr, opts).await?;
    // shadowsocks-rust falls back to IPv6 only when the port is taken over IPv4.
    if is_dual_stack_addr(&addr) && SockRef::from(&*listener).only_v6()? {
        return Err(io::Error::new(io::ErrorKind::AddrInUse, "the port is in use over IPv4"));
    }
    Ok(listener)
}

struct Inbound {
    context: SharedContext,
    server: ServerConfig,
    connect: ConnectOpts,
}

pub async fn serve(listener: TcpListener, server: ServerConfig) {
    let inbound = Arc::new(Inbound {
        context: Context::new_shared(ServerType::Server),
        server,
        connect: ConnectOpts {
            tcp: socket_opts(),
            ..Default::default()
        },
    });
    let slots = Arc::new(Semaphore::new(MAX_HANDSHAKES));
    let mut delay = Duration::ZERO;
    loop {
        let (stream, source) = match listener.accept().await {
            Ok(accepted) => accepted,
            // Such as running out of descriptors; logged once per run of failures, which would flood the log.
            Err(err) => {
                if delay.is_zero() {
                    error!("[TCP] accept error: {err}; retrying");
                }
                delay = (delay * 2).clamp(Duration::from_millis(5), Duration::from_secs(1));
                sleep(delay).await;
                continue;
            }
        };
        delay = Duration::ZERO;
        let source = SocketAddr::new(source.ip().to_canonical(), source.port());
        // Until the client is authenticated, closing resets the connection, as ssserver and sing-box do.
        _ = SockRef::from(&stream).set_linger(Some(Duration::ZERO));
        match slots.clone().try_acquire_owned() {
            Ok(slot) => _ = tokio::spawn(relay(inbound.clone(), stream, source, slot)),
            Err(_) => info!("[TCP] {source} rejected: too many handshakes"),
        }
    }
}

async fn relay(inbound: Arc<Inbound>, stream: TcpStream, source: SocketAddr, slot: OwnedSemaphorePermit) {
    let (mut client, target) = match timeout(HANDSHAKE_TIMEOUT, handshake(&inbound, stream)).await {
        Ok(Ok(accepted)) => accepted,
        Ok(Err(err)) => {
            info!("[TCP] {source} handshake error: {err}");
            return;
        }
        Err(_) => {
            info!("[TCP] {source} handshake error: timed out");
            return;
        }
    };
    drop(slot);
    _ = SockRef::from(client.get_ref().get_ref()).set_linger(None);

    let connect = OutboundTcpStream::connect_remote_with_opts(&inbound.context, &target, &inbound.connect);
    let mut remote = match timeout(DIAL_TIMEOUT, connect).await {
        Ok(Ok(remote)) => remote,
        Ok(Err(err)) => {
            warn!("[TCP] {source} --> {target} error: {err}");
            return;
        }
        Err(_) => {
            warn!("[TCP] {source} --> {target} error: timed out");
            return;
        }
    };
    info!("[TCP] {source} --> {target}");
    // Passes an EOF on as a half close, so the other direction keeps flowing.
    _ = copy_encrypted_bidirectional(inbound.server.method(), &mut client, &mut remote).await;
}

async fn handshake(inbound: &Inbound, stream: TcpStream) -> io::Result<(ProxyServerStream<ObfsStream>, Address)> {
    let stream = obfs::accept(stream).await?;
    let mut client = ProxyServerStream::from_stream(
        inbound.context.clone(),
        stream,
        inbound.server.method(),
        inbound.server.key(),
    );
    let target = client.handshake().await?;
    Ok((client, target))
}

// macOS accepts both binds.
#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn port_taken_over_ipv4() {
        let taken = std::net::TcpListener::bind("0.0.0.0:0").unwrap();
        let addr = SocketAddr::new(
            std::net::Ipv6Addr::UNSPECIFIED.into(),
            taken.local_addr().unwrap().port(),
        );
        assert_eq!(listen(addr).await.unwrap_err().kind(), io::ErrorKind::AddrInUse);
    }
}
