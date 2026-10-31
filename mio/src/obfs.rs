//! The HTTP obfuscation of simple-obfs: the client sends its first data as the body of a WebSocket
//! upgrade request and the server puts the upgrade response before its first data; both sides then
//! send raw data. The body is not buffered: the data after the request head is passed on as it comes.

use std::{
    io::{self, IoSlice},
    pin::Pin,
    sync::LazyLock,
    task::{Context, Poll, ready},
};

use aws_lc_rs::digest;
use base64::{Engine, engine::general_purpose::STANDARD};
use bytes::{Buf, BytesMut};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, ReadBuf},
    net::TcpStream,
};

const MAX_REQUEST_HEAD: usize = 8 << 10;
const WEBSOCKET_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

/// Stays the same for the whole process, like the version a real server reports.
static NGINX_VERSION: LazyLock<String> = LazyLock::new(|| {
    let [minor, patch] = random();
    format!("1.{}.{}", 20 + minor % 8, patch % 4)
});

pub struct ObfsStream {
    stream: TcpStream,
    /// Data read past the request head, until drained.
    early: BytesMut,
    /// The upgrade response, until sent with the first data.
    response: Vec<u8>,
}

/// Reads the request head of a simple-obfs client.
pub async fn accept(mut stream: TcpStream) -> io::Result<ObfsStream> {
    let mut head = BytesMut::with_capacity(MAX_REQUEST_HEAD);
    let end = loop {
        if head.len() >= MAX_REQUEST_HEAD {
            return Err(io::Error::other("request head too large"));
        }
        let from = head.len().saturating_sub(3);
        if stream.read_buf(&mut head).await? == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        if let Some(i) = head[from..].windows(4).position(|w| w == b"\r\n\r\n") {
            break from + i + 4;
        }
    };
    // A copy, so the buffer of the request head is freed even when no data follows it.
    let early = BytesMut::from(&head[end..]);
    let key = head[..end].split(|&b| b == b'\n').find_map(|line| {
        let (name, value) = std::str::from_utf8(line).ok()?.split_once(':')?;
        name.trim()
            .eq_ignore_ascii_case("sec-websocket-key")
            .then(|| value.trim())
    });
    Ok(ObfsStream {
        stream,
        early,
        response: upgrade_response(key),
    })
}

/// Answers the WebSocket key the way a real server does, with a random key for clients that send none.
fn upgrade_response(key: Option<&str>) -> Vec<u8> {
    let key = key.map_or_else(|| STANDARD.encode(random::<16>()), str::to_owned);
    let accept = digest::digest(
        &digest::SHA1_FOR_LEGACY_USE_ONLY,
        format!("{key}{WEBSOCKET_GUID}").as_bytes(),
    );
    format!(
        "HTTP/1.1 101 Switching Protocols\r\nServer: nginx/{}\r\nDate: {}\r\nConnection: upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Accept: {}\r\n\r\n",
        *NGINX_VERSION,
        crate::logger::now(c"%a, %d %b %Y %H:%M:%S GMT", true),
        STANDARD.encode(accept),
    )
    .into_bytes()
}

fn random<const N: usize>() -> [u8; N] {
    let mut bytes = [0; N];
    aws_lc_rs::rand::fill(&mut bytes).expect("system random number generator");
    bytes
}

impl ObfsStream {
    pub fn get_ref(&self) -> &TcpStream {
        &self.stream
    }

    /// Sends the rest of the response together with data; returns how much of data went out.
    fn poll_response(&mut self, cx: &mut Context<'_>, data: &[u8]) -> Poll<io::Result<usize>> {
        loop {
            let bufs = [IoSlice::new(&self.response), IoSlice::new(data)];
            let n = ready!(Pin::new(&mut self.stream).poll_write_vectored(cx, &bufs))?;
            if n == 0 {
                return Poll::Ready(Err(io::ErrorKind::WriteZero.into()));
            }
            if n < self.response.len() {
                self.response.drain(..n);
                continue;
            }
            let written = n - self.response.len();
            self.response = Vec::new();
            return Poll::Ready(Ok(written));
        }
    }
}

impl AsyncRead for ObfsStream {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        if this.early.is_empty() {
            return Pin::new(&mut this.stream).poll_read(cx, buf);
        }
        let n = this.early.len().min(buf.remaining());
        buf.put_slice(&this.early[..n]);
        this.early.advance(n);
        if this.early.is_empty() {
            // Frees the drained buffer.
            this.early = BytesMut::new();
        }
        Poll::Ready(Ok(()))
    }
}

impl AsyncWrite for ObfsStream {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        if !this.response.is_empty() {
            let n = ready!(this.poll_response(cx, buf))?;
            if n > 0 || buf.is_empty() {
                return Poll::Ready(Ok(n));
            }
        }
        Pin::new(&mut this.stream).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().stream).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        if !this.response.is_empty() {
            ready!(this.poll_response(cx, &[]))?;
        }
        Pin::new(&mut this.stream).poll_shutdown(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{
        io::AsyncWriteExt,
        net::{TcpListener, TcpStream},
    };

    async fn pair() -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap()).await.unwrap();
        (client, listener.accept().await.unwrap().0)
    }

    #[tokio::test]
    async fn upgrade() {
        let (mut client, server) = pair().await;
        client
            .write_all(
                b"GET / HTTP/1.1\r\nHost: example.com\r\nsec-websocket-key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\nfirst",
            )
            .await
            .unwrap();
        let mut server = accept(server).await.unwrap();
        let mut data = [0; 5];
        server.read_exact(&mut data).await.unwrap();
        assert_eq!(&data, b"first");

        server.write_all(b"reply").await.unwrap();
        server.write_all(b"more").await.unwrap();
        server.shutdown().await.unwrap();
        let mut response = String::new();
        client.read_to_string(&mut response).await.unwrap();
        assert!(
            response.starts_with("HTTP/1.1 101 Switching Protocols\r\n"),
            "{response}"
        );
        // The accept value of the sample handshake in RFC 6455.
        assert!(
            response.contains("\r\nSec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n"),
            "{response}"
        );
        assert!(response.ends_with("\r\n\r\nreplymore"), "{response}");
    }

    #[tokio::test]
    async fn response_without_data() {
        let (mut client, server) = pair().await;
        client.write_all(b"GET / HTTP/1.1\r\n\r\n").await.unwrap();
        let mut server = accept(server).await.unwrap();
        server.shutdown().await.unwrap();
        let mut response = String::new();
        client.read_to_string(&mut response).await.unwrap();
        assert!(
            response.starts_with("HTTP/1.1 101 ") && response.ends_with("\r\n\r\n"),
            "{response}"
        );
    }

    #[tokio::test]
    async fn head_too_large() {
        let (mut client, server) = pair().await;
        let head = format!("GET / HTTP/1.1\r\n{}", "X-Pad: 0123456789\r\n".repeat(500));
        tokio::spawn(async move { client.write_all(head.as_bytes()).await });
        assert!(accept(server).await.is_err());
    }
}
