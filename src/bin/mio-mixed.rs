// This program demonstrates how a single mio instance can be used to
// receive both system events (e.g. file descriptor events) and
// non-system events (e.g. events sourced on user-space threads other
// than the thread running the mio poll).  We listen for incoming UDP
// datagrams on port 2000, and also listen for events created by our
// timer thread every three seconds.
//
// Running this program on Linux via strace shows how mio notifies the
// polling thread of the non-system event by writing to a pipe:
//
// 28365 write(6, "\1", 1)                 = 1
// 28365 nanosleep({3, 0},  <unfinished ...>
// 28364 <... epoll_wait resumed> [{EPOLLIN, {u32=4294967295, u64=18446744073709551615}}], 16, -1) = 1
// 28364 read(5, "\1", 128)                = 1
// 28364 read(5, 0x7ffc96a72cf8, 128)      = -1 EAGAIN (Resource temporarily unavailable)
// 28364 write(1, "after poll\n", 11)      = 11
// 28364 write(1, "3-second timer\n", 15)  = 15

extern crate mio;

use std::io;
use std::thread;
use std::time::{Duration, Instant};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use mio::net::UdpSocket;
use mio::event::Evented;
use mio::{Events, Poll, PollOpt, Ready, Registration, Token};

const MAX_MESSAGE_SIZE: usize = 1500;
const MAX_EVENTS: usize = 16;
const ECHO_PORT: u16 = 2000;
const TIMER_INTERVAL_SECONDS: u64 = 3;

/// An Evented implementation which implements a simple timer by indicating readiness at periodic
/// intervals.
struct PeriodicTimer {
    registration: Registration,
    set_readiness: mio::SetReadiness,
}

impl PeriodicTimer {
    /// Create a PeriodicTimer and begin signalling readiness at the specified interval.
    fn new(interval: u64) -> PeriodicTimer {
        let (registration, set_readiness) = Registration::new2();
        let set_readiness_clone = set_readiness.clone();

        // Spawn a thread to periodically trigger the timer by setting read readiness.
        // Note: This example code omits important things like arranging termination of the thread
        // when the PeriodicTimer is dropped.
        thread::spawn(move || loop {
            let now = Instant::now();
            let when = now + Duration::from_secs(interval);
            if now < when {
                thread::sleep(when - now);
            }
            set_readiness_clone
                .set_readiness(Ready::readable())
                .unwrap();
        });
        PeriodicTimer {
            registration,
            set_readiness,
        }
    }

    /// Clear the read readiness of this timer.
    fn reset(&self) {
        self.set_readiness.set_readiness(Ready::empty()).unwrap();
    }
}

/// Proxy Evented functions to the Registration.
impl Evented for PeriodicTimer {
    fn register(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> io::Result<()> {
        self.registration.register(poll, token, interest, opts)
    }
    fn reregister(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> io::Result<()> {
        self.registration.reregister(poll, token, interest, opts)
    }
    fn deregister(&self, poll: &Poll) -> io::Result<()> {
        <Registration as Evented>::deregister(&self.registration, poll)
    }
}

fn main() {
    let localhost = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));

    // Create and bind the socket
    let socket = UdpSocket::bind(&SocketAddr::new(localhost, ECHO_PORT)).unwrap();

    // Set up mio polling
    let poll = Poll::new().unwrap();
    let mut events = Events::with_capacity(MAX_EVENTS);
    poll.register(&socket, Token(0), Ready::readable(), PollOpt::level())
        .unwrap();
    let timer = PeriodicTimer::new(TIMER_INTERVAL_SECONDS);
    poll.register(&timer, Token(1), Ready::readable(), PollOpt::level())
        .unwrap();

    // Main loop
    loop {
        // Poll
        println!("before poll()");
        poll.poll(&mut events, None).unwrap();
        println!("after poll()");

        // Process events
        for event in &events {
            assert!(event.token() == Token(0) || event.token() == Token(1));
            assert!(event.readiness().is_readable());
            match event.token() {
                Token(0) => {
                    let mut inbuf = [0u8; MAX_MESSAGE_SIZE];
                    let (nbytes, addr) = socket.recv_from(&mut inbuf).unwrap();
                    println!("recv {} bytes from {}.", nbytes, addr);
                }
                Token(1) => {
                    println!("{}-second timer", TIMER_INTERVAL_SECONDS);
                    timer.reset();
                }
                Token(_) => {
                    panic!("Unknown token in poll.");
                }
            }
        }
    }
}
