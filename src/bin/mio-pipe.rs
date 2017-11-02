// Demonstrate a possible bug where mio uses a pipe write to notify of
// a mio::Registration event which occurs while epoll_wait() is not
// happening.
//
// For more details, see:
// https://github.com/carllerche/mio/issues/785
//
// RESULTS:
// $ strace -f -s 1024 target/debug/mio-pipe
// ...
// pipe2([5, 6], O_NONBLOCK|O_CLOEXEC) = 0
// epoll_ctl(4, EPOLL_CTL_ADD, 5, {EPOLLIN|EPOLLET, {u32=4294967295, u64=18446744073709551615}}) = 0
// epoll_ctl(4, EPOLL_CTL_ADD, 3, {EPOLLIN, {u32=0, u64=0}}) = 0
// socket(PF_INET, SOCK_DGRAM|SOCK_CLOEXEC, IPPROTO_IP) = 7
// bind(7, {sa_family=AF_INET, sin_port=htons(0), sin_addr=inet_addr("127.0.0.1")}, 16) = 0
// sendto(7, "hello", 5, MSG_NOSIGNAL, {sa_family=AF_INET, sin_port=htons(2000), sin_addr=inet_addr("127.0.0.1")}, 16) = 5
// epoll_wait(4, [{EPOLLIN, {u32=0, u64=0}}], 16, -1) = 1
// recvfrom(3, "hello", 1500, 0, {sa_family=AF_INET, sin_port=htons(41815), sin_addr=inet_addr("127.0.0.1")}, [16]) = 5
// write(1, "recv 5 bytes from 127.0.0.1:41815.\n", 35) = 35
// write(6, "\1", 1)       = 1
// epoll_wait(4, [{EPOLLIN, {u32=4294967295, u64=18446744073709551615}}], 16, 0) = 1
// read(5, "\1", 128)      = 1
// read(5, 0x7fff70ace168, 128) = -1 EAGAIN (Resource temporarily unavailable)
// write(1, "mio::Registration readiness received.\n", 38) = 38
// ...

extern crate mio;

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use mio::net::UdpSocket;
use mio::{Events, Poll, PollOpt, Ready, Registration, Token};

fn main() {
    const MAX_MESSAGE_SIZE: usize = 1500;
    const MAX_EVENTS: usize = 16;
    const PORT: u16 = 2000;
    let localhost = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
    let send_address = SocketAddr::new(localhost, 0);
    let recv_address = SocketAddr::new(localhost, PORT);

    // Create and bind the socket
    let recv_socket = UdpSocket::bind(&recv_address).unwrap();

    // Set up mio polling
    let poll = Poll::new().unwrap();
    let mut events = Events::with_capacity(MAX_EVENTS);
    poll.register(&recv_socket, Token(0), Ready::readable(), PollOpt::level())
        .unwrap();
    let (registration, set_readiness) = Registration::new2();
    poll.register(&registration, Token(1), Ready::readable(), PollOpt::level())
        .unwrap();

    // Send a datagram to the listening socket.
    let send_socket = std::net::UdpSocket::bind(send_address).unwrap();
    send_socket
        .send_to("hello".as_bytes(), &recv_address)
        .unwrap();

    // Main loop
    'main_loop: loop {
        // Poll
        poll.poll(&mut events, None).unwrap();

        // Process events
        for event in &events {
            assert!(event.token() == Token(0) || event.token() == Token(1));
            assert!(event.readiness().is_readable());
            match event.token() {
                Token(0) => {
                    let mut inbuf = [0u8; MAX_MESSAGE_SIZE];
                    let (nbytes, addr) = recv_socket.recv_from(&mut inbuf).unwrap();
                    println!("recv {} bytes from {}.", nbytes, addr);
                    set_readiness.set_readiness(Ready::readable()).unwrap();
                }
                Token(1) => {
                    println!("mio::Registration readiness received.");
                    set_readiness.set_readiness(Ready::empty()).unwrap();
                    break 'main_loop;
                }
                Token(_) => {
                    panic!("Unknown token in poll.");
                }
            }
        }
    }
}
