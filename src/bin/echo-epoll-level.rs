// A simple UDP echo server using the Linux epoll facility to multiplex
// reads and writes.  This program uses level-triggered events, which
// makes it functionally similar to programs using epoll's predecessors
// select() and poll().

extern crate nix;

use std::collections::VecDeque;
use nix::sys::epoll::*;
use nix::sys::socket::*;

const MAX_MESSAGE_SIZE: usize = 1500;
const MAX_OUTGOING_MESSAGES: usize = 8;
const MAX_EVENTS: usize = 16;
const ECHO_PORT: u16 = 2000;

struct Message {
    buffer: Vec<u8>, // The contents of the message.
    addr: SockAddr, // The original source address (and echo destination).
}

fn main() {
    let localhost: IpAddr = IpAddr::new_v4(127, 0, 0, 1);

    // Open an IPv4 UDP socket in non-blocking mode.
    let socket_fd = socket(AddressFamily::Inet, SockType::Datagram, SOCK_NONBLOCK, 0).unwrap();
    // Bind the socket to IPv4 localhost, port 2000.
    bind(
        socket_fd,
        &SockAddr::new_inet(InetAddr::new(localhost, ECHO_PORT)),
    ).unwrap();

    // Create epoll events
    let mut event_read_only = EpollEvent::new(EPOLLIN, 0u64);
    let mut event_read_write = EpollEvent::new(EPOLLIN | EPOLLOUT, 0u64);
    let mut current_events = [EpollEvent::empty(); MAX_EVENTS];

    // Set up epoll
    let epoll_fd = epoll_create1(EpollCreateFlags::empty()).unwrap();
    epoll_ctl(
        epoll_fd,
        EpollOp::EpollCtlAdd,
        socket_fd,
        &mut event_read_only,
    ).unwrap();

    let mut outgoing_queue: VecDeque<Message> = VecDeque::new();
    loop {
        // Set up events
        if outgoing_queue.is_empty() {
            epoll_ctl(
                epoll_fd,
                EpollOp::EpollCtlMod,
                socket_fd,
                &mut event_read_only,
            ).unwrap();
        } else {
            epoll_ctl(
                epoll_fd,
                EpollOp::EpollCtlMod,
                socket_fd,
                &mut event_read_write,
            ).unwrap();
        }

        // Select via epoll
        let num_events = epoll_wait(epoll_fd, &mut current_events, -1).unwrap();

        // Process events
        for i in 0..num_events {
            let event = &current_events[i];
            if event.events().contains(EPOLLIN) {
                // Read from the socket.
                let mut inbuf = [0u8; MAX_MESSAGE_SIZE];
                let (nbytes, addr) = recvfrom(socket_fd, &mut inbuf).unwrap();
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
            if event.events().contains(EPOLLOUT) {
                // Write to the socket.
                let message = outgoing_queue.pop_front().unwrap();
                let nbytes = sendto(socket_fd, &message.buffer, &message.addr, MsgFlags::empty())
                    .unwrap();
                println!("sent {} bytes to {}.", nbytes, message.addr);
            }
        }
    }
}
