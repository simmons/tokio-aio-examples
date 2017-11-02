// A simple UDP echo server using the mio crate to multiplex reads and
// writes.  This program uses edge-triggered events, which can
// theoretically provide better performance than level-triggering by
// reducing the overhead related to selection.  Handlers are expected to
// perform as much I/O as possible until WouldBlock is indicated, at
// which time Poll::poll() is called again and other file descriptors
// may be handled.  Any mitigation of the edge-triggered starvation
// problem is up to the application, and no such mitigation is
// demonstrated here.

extern crate mio;

use std::collections::VecDeque;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use mio::net::UdpSocket;
use mio::{Events, Poll, PollOpt, Ready, Token};

const MAX_MESSAGE_SIZE: usize = 1500;
const MAX_OUTGOING_MESSAGES: usize = 8;
const MAX_EVENTS: usize = 16;
const ECHO_PORT: u16 = 2000;

struct Message {
    buffer: Vec<u8>, // The contents of the message.
    addr: SocketAddr, // The original source address (and echo destination).
}

fn main() {
    let localhost = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));

    // Open a UDP socket in non-blocking mode bound to IPv4 localhost port 2000.
    let socket = UdpSocket::bind(&SocketAddr::new(localhost, ECHO_PORT)).unwrap();

    // Set up mio polling
    let poll = Poll::new().unwrap();
    let mut events = Events::with_capacity(MAX_EVENTS);
    poll.register(&socket, Token(0), Ready::readable(), PollOpt::edge())
        .unwrap();

    // Main loop
    let mut can_read = true;
    let mut can_write = false;
    let mut outgoing_queue: VecDeque<Message> = VecDeque::new();
    loop {
        // Either read or write can set this to false to avoid a poll and re-run the loop
        // immediately.
        let mut blocking = true;

        // Try to read
        if can_read {
            let mut inbuf = [0u8; MAX_MESSAGE_SIZE];
            match socket.recv_from(&mut inbuf) {
                Ok((nbytes, addr)) => {
                    println!("recv {} bytes from {}.", nbytes, addr);
                    if outgoing_queue.len() > MAX_OUTGOING_MESSAGES {
                        println!("outgoing buffers exhausted; dropping packet.");
                    } else {
                        outgoing_queue.push_back(Message {
                            buffer: inbuf[0..nbytes].to_vec(),
                            addr,
                        });
                        println!("total pending writes: {}", outgoing_queue.len());

                        // Since we are edge-polling, we must at least try to write, and only poll
                        // for writability if the write returns a WouldBlock error.
                        can_write = true;
                    }
                    blocking = false;
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // Nothing to do
                }
                Err(e) => panic!("recvfrom: {}", e),
            };
        }

        // Try to write
        if can_write && !outgoing_queue.is_empty() {
            let message = outgoing_queue.pop_front().unwrap();
            match socket.send_to(&message.buffer, &message.addr) {
                Ok(nbytes) => {
                    println!("sent {} bytes to {}.", nbytes, message.addr);
                    blocking = false;
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // Return outgoing message to buffer
                    outgoing_queue.push_back(message);
                }
                Err(e) => panic!("sendto: {}", e),
            }
        }

        // If both read and write are returning WouldBlock, then poll.
        if blocking {
            // Set up events
            if outgoing_queue.is_empty() {
                poll.reregister(&socket, Token(0), Ready::readable(), PollOpt::edge())
                    .unwrap();
            } else {
                poll.reregister(
                    &socket,
                    Token(0),
                    Ready::readable() | Ready::writable(),
                    PollOpt::edge(),
                ).unwrap();
            }

            // Poll
            poll.poll(&mut events, None).unwrap();

            // Process events
            can_read = false;
            can_write = false;
            for event in &events {
                assert!(event.token() == Token(0));
                if event.readiness().is_readable() {
                    can_read = true;
                }
                if event.readiness().is_writable() {
                    can_write = true;
                }
            }
        }
    }
}
