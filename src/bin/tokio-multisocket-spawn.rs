// Receive data on multiple sockets: Spawn a task+future for each socket.
//
// This program listens for incoming UDP datagrams on IPv4 localhost
// ports 2000 through 2009, and prints a summary of each datagram to
// the standard output.
//
// This implementation works by creating ten futures, each added to the
// event loop within a distinct task via spawn().  This spawning will be
// performed by a UdpMultiServer future which is passed to Tokio as its
// main future via Core::run().
//
// In contrast to tokio-multisocket-join.rs, this approach avoids
// polling all futures when only a single future needs to be polled.
// For example, an incoming packet on port 2004 will produce output
// similar to the following:
//
// Future #4 poll()...
// recv 5 bytes from 127.0.0.1:60522 at 127.0.0.1:2004

extern crate futures;
#[macro_use]
extern crate tokio_core;

use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use futures::{Async, Future, Poll};
use tokio_core::net::UdpSocket;
use tokio_core::reactor::Core;
use tokio_core::reactor::Handle;

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

/// UdpMultiServer is a special future responsible for spawning a number of UdpServer futures, each
/// in their own task.
struct UdpMultiServer {
    handle: Handle,
    started: bool,
    sockets: Vec<UdpSocket>,
}

impl UdpMultiServer {
    fn new(handle: Handle) -> UdpMultiServer {
        UdpMultiServer {
            handle: handle,
            started: false,
            sockets: vec![],
        }
    }

    /// Add a socket to the list of sockets we will be managing.
    fn add(&mut self, socket: UdpSocket) {
        self.sockets.push(socket);
    }
}

impl Future for UdpMultiServer {
    type Item = ();
    type Error = io::Error;

    fn poll(&mut self) -> Poll<(), io::Error> {
        println!("UdpMultiServer::poll()");

        // If this is the first poll, spawn a future for each socket we will be managing.
        if !self.started {
            let mut id = 0usize;
            while !self.sockets.is_empty() {
                let socket = self.sockets.remove(0);

                // Create the future
                let future = UdpServer::new(socket, id).map_err(|_| ());
                id += 1;

                // Spawn the future so that it is handled in a distinct task, and thus can receive
                // notifications and be polled independently of other futures.
                self.handle.spawn(future);
            }

            self.started = true;
        }

        // Always return NotReady, since this future will never complete.
        Ok(Async::NotReady)
    }
}

fn main() {
    let localhost = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));

    // Create the tokio event loop
    let mut core = Core::new().unwrap();

    // Create the UdpMultiServer future and initialize it with NUM_SOCKETS sockets.
    let mut multi = UdpMultiServer::new(core.handle());
    for i in 0..NUM_SOCKETS {
        // Create and bind the socket
        let port = START_PORT + (i as u16);
        let socket = UdpSocket::bind(&SocketAddr::new(localhost, port), &core.handle()).unwrap();
        multi.add(socket);
    }

    // Run the tokio event loop
    core.run(multi).unwrap();
}
