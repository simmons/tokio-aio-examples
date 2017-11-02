// This is an "empty" mio example.  Mio is polled without having
// registered for any events, so the poll() never returns.  This can be
// useful for studying basic mio behaviors that occur regardless of any
// registrations.
//
// For example, when run via strace on Linux, we can see that mio always
// creates a pipe to accommodate non-system events sourced from
// user-space, and then configures the underlying epoll to watch for
// read events on the pipe:
//
// epoll_create1(EPOLL_CLOEXEC)            = 3
// pipe2([4, 5], O_NONBLOCK|O_CLOEXEC)     = 0
// epoll_ctl(3, EPOLL_CTL_ADD, 4, {EPOLLIN|EPOLLET, {u32=2^32-1, u64=2^64-1}}) = 0
// write(1, "Calling mio::Poll::poll()\n", 26) = 26
// epoll_wait(3, 0x7f417262b000, 16, -1)   = ...

extern crate mio;

use mio::{Events, Poll};

fn main() {
    const MAX_EVENTS: usize = 16;

    // Set up mio polling
    let poll = Poll::new().unwrap();
    let mut events = Events::with_capacity(MAX_EVENTS);

    // Poll
    println!("Calling mio::Poll::poll()");
    poll.poll(&mut events, None).unwrap();

    // Since we did not register for any events, the above poll() never returns.
    println!("This never happens.");
}
