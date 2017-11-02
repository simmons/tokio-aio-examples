// A simple UDP echo server using the cross-platform mio crate to
// multiplex reads and writes.  This program uses level-triggered
// events.

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
    poll.register(&socket, Token(0), Ready::readable(), PollOpt::level())
        .unwrap();

    // Main loop
    let mut outgoing_queue: VecDeque<Message> = VecDeque::new();
    loop {
        // Set up events
        if outgoing_queue.is_empty() {
            poll.reregister(&socket, Token(0), Ready::readable(), PollOpt::level())
                .unwrap();
        } else {
            poll.reregister(
                &socket,
                Token(0),
                Ready::readable() | Ready::writable(),
                PollOpt::level(),
            ).unwrap();
        }

        // Poll
        poll.poll(&mut events, None).unwrap();

        // Process events
        for event in &events {
            assert!(event.token() == Token(0));
            if event.readiness().is_readable() {
                // Read from the socket.
                let mut inbuf = [0u8; MAX_MESSAGE_SIZE];
                let (nbytes, addr) = socket.recv_from(&mut inbuf).unwrap();
                println!("recv {} bytes from {}.", nbytes, addr);

                // Echo by pushing the message to our outgoing queue.
                if outgoing_queue.len() > MAX_OUTGOING_MESSAGES {
                    println!("outgoing buffers exhausted; dropping packet.");
                } else {
                    outgoing_queue.push_back(Message {
                        buffer: inbuf[0..nbytes].to_vec(),
                        addr,
                    });
                    println!("total pending writes: {}", outgoing_queue.len());
                }
            }
            if event.readiness().is_writable() {
                // Write to the socket.
                let message = outgoing_queue.pop_front().unwrap();
                let nbytes = socket.send_to(&message.buffer, &message.addr).unwrap();
                println!("sent {} bytes to {}.", nbytes, message.addr);
            }
        }
    }
}
