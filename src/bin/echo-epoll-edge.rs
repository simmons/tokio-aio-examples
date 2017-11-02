// A simple UDP echo server using the Linux epoll facility to multiplex
// reads and writes.  This program uses edge-triggered events, which can
// theoretically provide better performance than level-triggering by
// reducing the overhead related to selection.  Handlers are expected to
// perform as much I/O as possible until an EWOULDBLOCK is indicated, at
// which time epoll_wait() is called again and other file descriptors
// may be handled.  Any mitigation of the edge-triggered starvation
// problem is up to the application, and no such mitigation is
// demonstrated here.

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
    let mut event_read_only = EpollEvent::new(EPOLLIN | EPOLLET, 0u64);
    let mut event_read_write = EpollEvent::new(EPOLLIN | EPOLLOUT | EPOLLET, 0u64);
    let mut current_events = [EpollEvent::empty(); MAX_EVENTS];

    // Set up epoll
    let epoll_fd = epoll_create1(EpollCreateFlags::empty()).unwrap();
    epoll_ctl(
        epoll_fd,
        EpollOp::EpollCtlAdd,
        socket_fd,
        &mut event_read_only,
    ).unwrap();

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
            match recvfrom(socket_fd, &mut inbuf) {
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
                        // for writability if the write returns EWOULDBLOCK.
                        can_write = true;
                    }
                    blocking = false;
                }
                Err(nix::Error::Sys(errno)) if errno == nix::errno::EWOULDBLOCK => {
                    // Nothing to do
                }
                Err(e) => panic!("recvfrom: {}", e),
            };
        }

        // Try to write
        if can_write && !outgoing_queue.is_empty() {
            let message = outgoing_queue.pop_front().unwrap();
            match sendto(socket_fd, &message.buffer, &message.addr, MsgFlags::empty()) {
                Ok(nbytes) => {
                    println!("sent {} bytes to {}.", nbytes, message.addr);
                    blocking = false;
                }
                Err(nix::Error::Sys(errno)) if errno == nix::errno::EWOULDBLOCK => {
                    // Return outgoing message to buffer
                    outgoing_queue.push_back(message);
                }
                Err(e) => panic!("sendto: {}", e),
            }
        }

        // If both read and write are returning WouldBlock, then epoll_wait().
        if blocking {
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
            println!("before wait");
            let num_events = epoll_wait(epoll_fd, &mut current_events, -1).unwrap();
            println!("after wait");

            // Process events
            can_read = false;
            can_write = false;
            for i in 0..num_events {
                if current_events[i].events().contains(EPOLLIN) {
                    can_read = true;
                }
                if current_events[i].events().contains(EPOLLOUT) {
                    can_write = true;
                }
            }
        }
    }
}
