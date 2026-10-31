//! mio runs a Shadowsocks 2022 server behind the HTTP obfuscation of simple-obfs and sends all traffic out directly.

mod config;
mod logger;
mod obfs;
mod tcp;

use std::{io, net::SocketAddr, process::exit, time::Duration};

use log::{error, info};
use shadowsocks::{ServerConfig, config::Mode, net::is_dual_stack_addr};
use shadowsocks_service::server::ServerBuilder;
use socket2::{Domain, Socket, Type};
use tokio::{
    signal::unix::{SignalKind, signal},
    task::JoinHandle,
};

use crate::config::Config;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

const VERSION: &str = match option_env!("MIO_VERSION") {
    Some(version) => version,
    None => "dev",
};
const UDP_TIMEOUT: Duration = Duration::from_secs(60);
/// Beyond this the least recently used session is dropped, so UDP cannot use up the descriptors.
const UDP_SESSIONS: usize = 1024;
const USAGE: &str = "usage: mio -c <config> -o <log file> [-l warning|info] [-t]\n       mio -v";

#[derive(Default)]
struct Args {
    config: Option<String>,
    level: Option<String>,
    output: Option<String>,
    test: bool,
    version: bool,
}

fn parse_args() -> Option<Args> {
    let mut args = Args::default();
    let mut iter = std::env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-c" => args.config = Some(iter.next()?),
            "-l" => args.level = Some(iter.next()?),
            "-o" => args.output = Some(iter.next()?),
            "-t" => args.test = true,
            "-v" => args.version = true,
            _ => return None,
        }
    }
    (args.version || args.config.is_some() && args.output.is_some()).then_some(args)
}

struct Server {
    addr: SocketAddr,
    tasks: [JoinHandle<()>; 2],
}

/// Serves TCP and UDP on the same port.
async fn start(config: &Config) -> io::Result<Server> {
    let listener = tcp::listen(config.listen).await?;
    let addr = listener.local_addr()?;
    check_udp(addr)?;
    let mut server = ServerConfig::new(addr, config.password.clone(), config.method).map_err(io::Error::other)?;
    server.set_mode(Mode::UdpOnly);
    let mut udp = ServerBuilder::new(server.clone());
    udp.set_udp_expiry_duration(UDP_TIMEOUT);
    udp.set_udp_capacity(UDP_SESSIONS);
    let udp = udp.build().await?;
    Ok(Server {
        addr,
        tasks: [
            tokio::spawn(tcp::serve(listener, server)),
            tokio::spawn(async move {
                if let Err(err) = udp.run().await {
                    error!("UDP server stopped: {err}");
                }
            }),
        ],
    })
}

/// shadowsocks-rust binds UDP for IPv6 only when the port is taken over IPv4 and hides its socket,
/// so the port is tried first, to fail as TCP does.
fn check_udp(addr: SocketAddr) -> io::Result<()> {
    if !is_dual_stack_addr(&addr) {
        return Ok(());
    }
    let socket = Socket::new(Domain::IPV6, Type::DGRAM, None)?;
    socket.set_only_v6(false)?;
    socket.bind(&addr.into())
}

fn main() {
    let Some(args) = parse_args() else {
        eprintln!("{USAGE}");
        exit(2);
    };
    if args.version {
        println!("mio {VERSION}");
        return;
    }

    if let Err(err) = logger::init(args.output.as_deref().unwrap_or_default()) {
        eprintln!("Failed to open log file: {err}");
        exit(1);
    }
    let level = args.level.as_deref().unwrap_or("warning");
    let Some(level) = logger::parse_level(level) else {
        error!("Invalid log level: {level}");
        exit(1);
    };
    log::set_max_level(level);

    let config = match Config::load(args.config.as_deref().unwrap_or_default()) {
        Ok(config) => config,
        Err(err) => {
            error!("Invalid config: {err}");
            exit(1);
        }
    };
    if args.test {
        return;
    }

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap_or_else(|err| {
            error!("Failed to start: {err}");
            exit(1);
        });
    runtime.block_on(async {
        let mut server = start(&config).await.unwrap_or_else(|err| {
            error!("Failed to start: {err}");
            exit(1);
        });
        let (Ok(mut terminate), Ok(mut interrupt)) = (signal(SignalKind::terminate()), signal(SignalKind::interrupt()))
        else {
            error!("Failed to start: unable to handle signals");
            exit(1);
        };
        info!("mio {VERSION} started: TCP and UDP on {}", server.addr);
        let [tcp, udp] = &mut server.tasks;
        tokio::select! {
            _ = terminate.recv() => {}
            _ = interrupt.recv() => {}
            // A server stops only on a fatal error or a panic; procd restarts mio.
            _ = tcp => exit(1),
            _ = udp => exit(1),
        }
    });
    // Does not wait for name lookups in progress.
    runtime.shutdown_background();
}

#[cfg(test)]
mod tests {
    use std::{
        pin::Pin,
        task::{Context, Poll, ready},
    };

    use shadowsocks::{
        config::ServerType,
        context::Context as SsContext,
        crypto::CipherKind,
        relay::{
            socks5::Address,
            tcprelay::ProxyClientStream,
            udprelay::{ProxySocket, options::UdpSocketControlData},
        },
    };
    use tokio::{
        io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf},
        net::{TcpListener, TcpStream, UdpSocket},
        time::timeout,
    };

    use super::*;

    const KEY: &str = "tZr561EnTV6LPmRm+ONfCw==";
    const METHOD: CipherKind = CipherKind::AEAD2022_BLAKE3_AES_128_GCM;

    async fn server() -> (Server, ServerConfig) {
        let config = Config {
            listen: "127.0.0.1:0".parse().unwrap(),
            password: KEY.into(),
            method: METHOD,
        };
        let server = start(&config).await.unwrap();
        let client = ServerConfig::new(server.addr, KEY, METHOD).unwrap();
        (server, client)
    }

    /// The client side of the obfuscation: the request head goes first, the response head is skipped.
    struct ObfsClient {
        stream: TcpStream,
        head: Option<Vec<u8>>,
        rest: Vec<u8>,
    }

    impl ObfsClient {
        async fn connect(addr: SocketAddr) -> Self {
            let mut stream = TcpStream::connect(addr).await.unwrap();
            stream
                .write_all(b"GET / HTTP/1.1\r\nHost: example.com\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n")
                .await
                .unwrap();
            Self {
                stream,
                head: Some(Vec::new()),
                rest: Vec::new(),
            }
        }
    }

    impl AsyncRead for ObfsClient {
        fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
            let this = self.get_mut();
            while let Some(head) = &mut this.head {
                let mut chunk = [0; 1024];
                let mut chunk = ReadBuf::new(&mut chunk);
                ready!(Pin::new(&mut this.stream).poll_read(cx, &mut chunk))?;
                if chunk.filled().is_empty() {
                    return Poll::Ready(Err(io::ErrorKind::UnexpectedEof.into()));
                }
                head.extend_from_slice(chunk.filled());
                if let Some(i) = head.windows(4).position(|w| w == b"\r\n\r\n") {
                    this.rest = head.split_off(i + 4);
                    this.head = None;
                }
            }
            if this.rest.is_empty() {
                return Pin::new(&mut this.stream).poll_read(cx, buf);
            }
            let n = this.rest.len().min(buf.remaining());
            buf.put_slice(&this.rest[..n]);
            this.rest.drain(..n);
            Poll::Ready(Ok(()))
        }
    }

    impl AsyncWrite for ObfsClient {
        fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
            Pin::new(&mut self.get_mut().stream).poll_write(cx, buf)
        }

        fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Pin::new(&mut self.get_mut().stream).poll_flush(cx)
        }

        fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Pin::new(&mut self.get_mut().stream).poll_shutdown(cx)
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn tcp_half_close() {
        let (server, config) = server().await;
        // Answers only after the client has finished sending.
        let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let target_addr = target.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut conn, _) = target.accept().await.unwrap();
            let mut data = Vec::new();
            conn.read_to_end(&mut data).await.unwrap();
            conn.write_all(&data).await.unwrap();
        });

        let context = SsContext::new_shared(ServerType::Local);
        let stream = ObfsClient::connect(server.addr).await;
        let mut client = ProxyClientStream::from_stream(context, stream, &config, target_addr);
        let payload: Vec<u8> = (0..300_000u32).map(|i| i as u8).collect();
        // The first write carries the Shadowsocks request header.
        client.write_all(&payload[..1000]).await.unwrap();
        client.write_all(&payload[1000..]).await.unwrap();
        client.shutdown().await.unwrap();
        let mut reply = Vec::new();
        timeout(Duration::from_secs(5), client.read_to_end(&mut reply))
            .await
            .unwrap()
            .unwrap();
        assert!(reply == payload, "echoed {} of {} bytes", reply.len(), payload.len());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn tcp_probe_is_reset() {
        let (server, _) = server().await;
        let mut probe = TcpStream::connect(server.addr).await.unwrap();
        probe.write_all(b"GET / HTTP/1.1\r\n\r\n").await.unwrap();
        probe.write_all(&[7; 100]).await.unwrap();
        let mut buffer = [0; 64];
        let result = timeout(Duration::from_secs(5), probe.read(&mut buffer)).await.unwrap();
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::ConnectionReset);
    }

    // macOS accepts both binds.
    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn udp_port_taken_over_ipv4() {
        let taken = std::net::UdpSocket::bind("0.0.0.0:0").unwrap();
        let config = Config {
            listen: SocketAddr::new(
                std::net::Ipv6Addr::UNSPECIFIED.into(),
                taken.local_addr().unwrap().port(),
            ),
            password: KEY.into(),
            method: METHOD,
        };
        assert_eq!(start(&config).await.err().unwrap().kind(), io::ErrorKind::AddrInUse);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn udp() {
        let (_server, config) = server().await;
        let target = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let target_addr = target.local_addr().unwrap();
        tokio::spawn(async move {
            let mut buffer = [0; 2048];
            while let Ok((n, from)) = target.recv_from(&mut buffer).await {
                _ = target.send_to(&buffer[..n], from).await;
            }
        });

        let context = SsContext::new_shared(ServerType::Local);
        let client = ProxySocket::connect(context, &config).await.unwrap();
        let destination = Address::SocketAddress(target_addr);
        let mut control = UdpSocketControlData::default();
        control.client_session_id = 0x6d696f;
        let mut buffer = [0; 2048];
        for payload in [&b"hello"[..], b"again"] {
            control.packet_id += 1;
            client.send_with_ctrl(&destination, &control, payload).await.unwrap();
            let (n, source, _) = timeout(Duration::from_secs(5), client.recv(&mut buffer))
                .await
                .unwrap()
                .unwrap();
            assert_eq!(&buffer[..n], payload);
            assert_eq!(source, destination);
        }
    }
}
