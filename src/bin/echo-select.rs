// A simple UDP echo server using POSIX select() to multiplex reads and
// writes.  This program can only be compiled on platforms which support
// select() (Mac, Linux, etc.).

extern crate nix;

use std::collections::VecDeque;
use nix::sys::select::*;
use nix::sys::socket::*;

const MAX_MESSAGE_SIZE: usize = 1500;
const MAX_OUTGOING_MESSAGES: usize = 8;
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

    let mut outgoing_queue: VecDeque<Message> = VecDeque::new();
    let mut read_fd_set = FdSet::new();
    let mut write_fd_set = FdSet::new();
    loop {
        // Set up read/write file descriptor sets
        read_fd_set.clear();
        read_fd_set.insert(socket_fd);
        write_fd_set.clear();
        if !outgoing_queue.is_empty() {
            write_fd_set.insert(socket_fd);
        }

        // Wait for the socket to be ready for reading
        // (and/or writing, if there are outgoing packets to send).
        select(
            socket_fd + 1,
            Some(&mut read_fd_set),
            Some(&mut write_fd_set),
            None,
            None,
        ).unwrap();

        // Process events.
        if read_fd_set.contains(socket_fd) {
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
            }
        }
        if write_fd_set.contains(socket_fd) {
            // Write to the socket.
            let message = outgoing_queue.pop_front().unwrap();
            let nbytes = sendto(socket_fd, &message.buffer, &message.addr, MsgFlags::empty())
                .unwrap();
            println!("sent {} bytes to {}.", nbytes, message.addr);
        }
    }
}
