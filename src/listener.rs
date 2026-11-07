use std::{
    future::Future,
    io,
    net::SocketAddr,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};

use axum::{
    extract::connect_info::Connected,
    serve::{IncomingStream, Listener},
};
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    net::{TcpListener, TcpStream},
    sync::{OwnedSemaphorePermit, Semaphore},
    time::{Instant, Sleep, sleep},
};

const CONNECTION_LIMIT: usize = 256;
const READ_IDLE_TIMEOUT: Duration = Duration::from_secs(60);
const WRITE_IDLE_TIMEOUT: Duration = Duration::from_secs(30);
const CONNECTION_LIFETIME: Duration = Duration::from_secs(300);

pub struct BoundedListener {
    listener: TcpListener,
    permits: Arc<Semaphore>,
}

impl BoundedListener {
    pub fn new(listener: TcpListener) -> Self {
        Self {
            listener,
            permits: Arc::new(Semaphore::new(CONNECTION_LIMIT)),
        }
    }
}

impl Listener for BoundedListener {
    type Io = Connection<TcpStream>;
    type Addr = SocketAddr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        let permit = self
            .permits
            .clone()
            .acquire_owned()
            .await
            .expect("connection limiter remains open");
        let (socket, address) = Listener::accept(&mut self.listener).await;
        (Connection::new(socket, permit), address)
    }

    fn local_addr(&self) -> io::Result<Self::Addr> {
        self.listener.local_addr()
    }
}

#[derive(Clone, Copy)]
pub struct PeerAddress(pub SocketAddr);

impl Connected<IncomingStream<'_, BoundedListener>> for PeerAddress {
    fn connect_info(stream: IncomingStream<'_, BoundedListener>) -> Self {
        Self(*stream.remote_addr())
    }
}

pub struct Connection<IO> {
    io: IO,
    read: IdleDeadline,
    write: IdleDeadline,
    lifetime: Pin<Box<Sleep>>,
    _permit: OwnedSemaphorePermit,
}

impl<IO> Connection<IO> {
    fn new(io: IO, permit: OwnedSemaphorePermit) -> Self {
        Self {
            io,
            read: IdleDeadline::new(READ_IDLE_TIMEOUT),
            write: IdleDeadline::new(WRITE_IDLE_TIMEOUT),
            lifetime: Box::pin(sleep(CONNECTION_LIFETIME)),
            _permit: permit,
        }
    }

    fn check_lifetime(&mut self, context: &mut Context<'_>) -> io::Result<()> {
        if self.lifetime.as_mut().poll(context).is_ready() {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "connection lifetime exceeded",
            ));
        }
        Ok(())
    }
}

impl<IO: AsyncRead + Unpin> AsyncRead for Connection<IO> {
    fn poll_read(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        this.check_lifetime(context)?;
        let result = Pin::new(&mut this.io).poll_read(context, buffer);
        this.read.check(context, result)
    }
}

impl<IO: AsyncWrite + Unpin> AsyncWrite for Connection<IO> {
    fn poll_write(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        this.check_lifetime(context)?;
        let result = Pin::new(&mut this.io).poll_write(context, buffer);
        this.write.check(context, result)
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffers: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        this.check_lifetime(context)?;
        let result = Pin::new(&mut this.io).poll_write_vectored(context, buffers);
        this.write.check(context, result)
    }

    fn is_write_vectored(&self) -> bool {
        self.io.is_write_vectored()
    }

    fn poll_flush(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        Pin::new(&mut this.io).poll_flush(context)
    }

    fn poll_shutdown(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().io).poll_shutdown(context)
    }
}

struct IdleDeadline {
    duration: Duration,
    sleep: Pin<Box<Sleep>>,
    waiting: bool,
}

impl IdleDeadline {
    fn new(duration: Duration) -> Self {
        Self {
            duration,
            sleep: Box::pin(sleep(duration)),
            waiting: false,
        }
    }

    fn check<T>(
        &mut self,
        context: &mut Context<'_>,
        result: Poll<io::Result<T>>,
    ) -> Poll<io::Result<T>> {
        if result.is_ready() {
            self.waiting = false;
            return result;
        }

        // Poll the deadline even if Hyper's response body is blocked by TCP backpressure.
        if !self.waiting {
            self.sleep.as_mut().reset(Instant::now() + self.duration);
            self.waiting = true;
        }
        if self.sleep.as_mut().poll(context).is_ready() {
            return Poll::Ready(Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "socket idle timeout",
            )));
        }
        Poll::Pending
    }
}

#[cfg(test)]
mod tests {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use super::*;

    #[tokio::test]
    async fn times_out_a_socket_that_stops_accepting_writes() {
        let permits = Arc::new(Semaphore::new(1));
        let (socket, _peer) = tokio::io::duplex(8);
        let mut connection = Connection::new(socket, permits.clone().try_acquire_owned().unwrap());
        connection.write = IdleDeadline::new(Duration::from_millis(10));

        let result = tokio::time::timeout(Duration::from_secs(2), connection.write_all(&[0; 16]))
            .await
            .unwrap();
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::TimedOut);
        assert_eq!(permits.available_permits(), 0);
        drop(connection);
        assert_eq!(permits.available_permits(), 1);
    }

    #[tokio::test]
    async fn times_out_an_idle_socket_read() {
        let permits = Arc::new(Semaphore::new(1));
        let (socket, _peer) = tokio::io::duplex(8);
        let mut connection = Connection::new(socket, permits.try_acquire_owned().unwrap());
        connection.read = IdleDeadline::new(Duration::from_millis(10));

        let result = tokio::time::timeout(Duration::from_secs(2), connection.read_u8())
            .await
            .unwrap();
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::TimedOut);
    }
}
