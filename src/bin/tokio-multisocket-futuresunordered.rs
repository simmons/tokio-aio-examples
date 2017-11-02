// Receive data on multiple sockets: Future composition with FuturesUnordered
//
// This program listens for incoming UDP datagrams on IPv4 localhost
// ports 2000 through 2009, and prints a summary of each datagram to
// the standard output.
//
// This implementation works by creating ten futures, each processing
// data on one socket, and managing them with a FuturesUnordered stream,
// which is provided to Tokio's Core::run() by way of the stream's
// for_each() method.
//
// FuturesUnordered has a very useful property that makes it potentially
// more attractive (in this scenario) than a simple join.  A Join
// future, when polled, will in turn poll all of its active futures,
// even if only one future needs to be polled (i.e., only one future
// arranged a notification event for the task which contains the Join
// and its child futures).  In contrast, "Futures managed by
// FuturesUnordered will only be polled when they generate
// notifications" [1].  This is accomplished by FuturesUnordered::poll()
// polling each future with a distinct NotifyHandle thread-local, so it
// can perform per-future notification discrimination and only poll the
// futures that need to be polled.
//
// [1] https://docs.rs/futures/0.1.18/futures/stream/futures_unordered/struct.FuturesUnordered.html
//
// When this program is run, you can observe that only the correct
// future is polled.  For example, an incoming packet on port 2004 will
// produce output similar to the following:
//
// Future #4 poll()...
// recv 5 bytes from 127.0.0.1:60522 at 127.0.0.1:2004
//

extern crate futures;
#[macro_use]
extern crate tokio_core;

use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use futures::{Future, Poll, Stream};
use futures::stream::FuturesUnordered;
use tokio_core::net::UdpSocket;
use tokio_core::reactor::Core;

const NUM_SOCKETS: usize = 10;
const START_PORT: u16 = 2000;

struct UdpServer {
    socket: UdpSocket,
    id: usize
}

impl UdpServer {
    fn new(socket: UdpSocket, id: usize) -> UdpServer {
        UdpServer { socket, id }
    }
}

impl Future for UdpServer {
    type Item = ();
    type Error = io::Error;

    fn poll(&mut self) -> Poll<(), io::Error> {
        println!("Future #{} poll()...", self.id);
        let mut buffer = vec![0; 1024];
        loop {
            let (nbytes, addr) = try_nb!(self.socket.recv_from(&mut buffer));
            println!(
                "recv {} bytes from {} at {}",
                nbytes,
                addr,
                self.socket.local_addr().unwrap()
            );
        }
    }
}

fn main() {
    let localhost = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));

    // Create the tokio event loop
    let mut core = Core::new().unwrap();

    // Create a future for each port, and add them to the FuturesUnordered set.
    let mut future_set = FuturesUnordered::<UdpServer>::new();
    for i in 0..NUM_SOCKETS {
        // Create and bind the socket
        let port = START_PORT + (i as u16);
        let socket = UdpSocket::bind(&SocketAddr::new(localhost, port), &core.handle()).unwrap();

        // Create the future
        let server = UdpServer::new(socket, i);

        // Add the future to the FuturesUnordered set.
        future_set.push(server);
    }

    // Create a future that consumes the FuturesUnordered stream.
    let future = future_set.for_each(|()| {
        Ok(())
    });

    // Run the tokio event loop
    core.run(future).unwrap();
}
