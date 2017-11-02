// Receive data on multiple sockets: Future composition via join_all().
//
// This program listens for incoming UDP datagrams on IPv4 localhost
// ports 2000 through 2009, and prints a summary of each datagram to
// the standard output.
//
// This implementation works by creating ten futures, each processing
// data on one socket, and combining them into a single composite future
// via join_all().  This composite future is provided to Tokio via
// Core::run().  This is a simple approach, but a possible downside is
// that every future is polled whenever a single packet arrives on a
// socket.  This is because all the futures run within a single task.
// Because notifications happen at the task level, any notification
// arranged in any of the futures will cause the main task to be
// notified.  It will poll the top-level FromAll future, which itself
// will poll each of its children.
//
// For example, an incoming packet on port 2004 will produce output
// similar to the following:
//
// Future #0 poll()...
// Future #1 poll()...
// Future #2 poll()...
// Future #3 poll()...
// Future #4 poll()...
// recv 5 bytes from 127.0.0.1:60522 at 127.0.0.1:2004
// Future #5 poll()...
// Future #6 poll()...
// Future #7 poll()...
// Future #8 poll()...
// Future #9 poll()...
//
// For an alternative approach, see tokio-multisocket-spawn.rs.

extern crate futures;
#[macro_use]
extern crate tokio_core;

use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use futures::{future, Future, Poll};
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

    // Create a future for each port
    let mut socket_futures = vec![];
    for i in 0..NUM_SOCKETS {
        // Create and bind the socket
        let port = START_PORT + (i as u16);
        let socket = UdpSocket::bind(&SocketAddr::new(localhost, port), &core.handle()).unwrap();

        // Create the future
        let server = UdpServer::new(socket, i);

        socket_futures.push(server);
    }

    // Combine the futures via join
    let future = future::join_all(socket_futures);

    // Run the tokio event loop
    core.run(future).unwrap();
}
