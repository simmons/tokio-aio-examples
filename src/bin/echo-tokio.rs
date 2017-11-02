// A simple UDP echo server using Tokio to multiplex reads and writes.
//
// This is similar to tokio-core's echo-udp.rs example program, but it
// performs true multiplexing of reads and writes through the use of an
// outgoing packet queue.  Incoming messages may be received and
// processed while outgoing writes are pending.  This is in contrast to
// the flip-flop operation of echo-udp.rs where the program is either in
// a sending state or receiving state at any given point in time.
//
// For reference, the tokio-core echo-udp.rs source may be found here:
// https://github.com/tokio-rs/tokio-core/blob/master/examples/echo-udp.rs

extern crate futures;
extern crate tokio_core;

use std::io;
use std::collections::VecDeque;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use futures::{Async, Future, Poll};
use tokio_core::net::UdpSocket;
use tokio_core::reactor::Core;

const MAX_MESSAGE_SIZE: usize = 1500;
const MAX_OUTGOING_MESSAGES: usize = 8;
const ECHO_PORT: u16 = 2000;

struct Message {
    buffer: Vec<u8>, // The contents of the message.
    addr: SocketAddr, // The original source address (and echo destination).
}

struct UdpServer {
    socket: UdpSocket,
    outgoing_queue: VecDeque<Message>,
}

impl UdpServer {
    fn new(socket: UdpSocket) -> UdpServer {
        UdpServer {
            socket: socket,
            outgoing_queue: VecDeque::new(),
        }
    }
}

impl Future for UdpServer {
    type Item = ();
    type Error = io::Error;

    // Read and write as needed, storing read packets in an outgoing queue for later writing.
    // We avoid the try_nb! macro here (and its potential for early return) so that a WouldBlock on
    // either the reading or writing doesn't prevent progress on the other.
    fn poll(&mut self) -> Poll<(), io::Error> {
        let (mut read, mut write) = (true, true);

        // Loop until no progress can be made on either reading or writing.
        while read || write {

            // If an outgoing buffer is present, try to send it.
            if let Some(message) = self.outgoing_queue.pop_front() {
                match self.socket.send_to(&message.buffer, &message.addr) {
                    Ok(nbytes) => {
                        println!("sent {} bytes to {}", nbytes, message.addr);
                    }
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                        // Writing would block -- re-queue this buffer and move on to reading.
                        println!("sending would block; defer.");
                        self.outgoing_queue.push_front(message);
                        write = false;
                    }
                    Err(e) => {
                        return Err(e);
                    }
                }
            } else {
                write = false;
            }

            // Read from the socket, if possible.
            let mut buffer = vec![0; MAX_MESSAGE_SIZE];
            match self.socket.recv_from(&mut buffer) {
                Ok((nbytes, addr)) => {
                    println!("recv {} bytes from {}", nbytes, addr);

                    if self.outgoing_queue.len() > MAX_OUTGOING_MESSAGES {
                        println!("outgoing buffers exhausted; dropping packet.");
                    } else {
                        // Trim the buffer.
                        buffer.truncate(nbytes);
                        // Push this buffer to the outgoing queue.
                        self.outgoing_queue.push_back(Message { addr, buffer });
                        write = true;
                    }
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    // Reading would block -- re-queue this buffer and move on to reading.
                    println!("reading would block.");
                    read = false;
                }
                Err(e) => {
                    return Err(e);
                }
            }

        }
        return Ok(Async::NotReady);
    }
}

fn main() {
    let localhost = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));

    // Create the tokio event loop
    let mut core = Core::new().unwrap();

    // Open a UDP socket in non-blocking mode bound to IPv4 localhost port 2000.
    let socket = UdpSocket::bind(&SocketAddr::new(localhost, ECHO_PORT), &core.handle()).unwrap();

    // Create the future
    let server = UdpServer::new(socket);

    // Run the tokio event loop
    core.run(server).unwrap();
}
