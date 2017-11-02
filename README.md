
Tokio asynchronous I/O examples
========================================================================

While studying the inner workings of Tokio, Mio, and futures, I
developed several small example programs to better understand usage
and explore somewhat different approaches or scenarios than the example
programs provided with `tokio-core`.  These example programs are based
on (at most) `tokio-core` and not higher-level crates like `tokio-proto`
and `tokio-service`.

I documented my conclusions from this study in a blog post:

[Tokio internals: Understanding Rust's asynchronous I/O framework from the bottom up](https://cafbit.com/post/tokio_internals/)

UDP echo examples
----------------------------------------

The `tokio-core` `echo-udp.rs` example program operates in a flip-flop
fashion where it is either listening for an incoming datagram, or
sending an outgoing datagram.  While it is waiting for a send of an
outgoing datagram to complete, it cannot process any pending incoming
datagrams.  This flip-flop behavior is probably fine for many protocols
like DNS and NTP, since a certain amount of incoming packets will be
buffered in the kernel queue.  Other more complex protocols or scenarios
may require true multiplexing of reads and writes.

I wrote a series of small UDP echo example programs that listen on IPv4
localhost port 2000, maintain a small outgoing queue for the echos, and
multiplex reads and writes.  I started with implementations based on the
operating system `select()` and `epoll` facilities, and worked my way up
to Mio and Tokio implementations.

- `echo-select.rs`:
This implementation uses the `select()` system call to manage I/O.
This only works on systems supporting `select()` (Linux, Mac OS, etc.)
and is only compiled when the `select` feature flag is given.

- `echo-epoll-level.rs`:
This implementation uses the Linux `epoll` facility in level-triggered
mode as a "better select".

- `echo-epoll-edge.rs`:
This implementation uses the Linux `epoll` facility in edge-triggered
mode.  Edge-triggered events can theoretically provide better
performance than level-triggering by reducing the overhead related to
selection.  Handlers are expected to perform as much I/O as possible
until an `EWOULDBLOCK` is indicated, at which time `epoll_wait()` is
called again and other file descriptors may be handled.  Any mitigation
of the edge-triggered starvation problem is up to the application, and
no such mitigation is demonstrated here.

- `echo-mio-level.rs`:
A simple UDP echo server using the cross-platform `mio` crate to
multiplex reads and writes.  This program uses level-triggered events.

- `echo-mio-edge.rs`:
A simple UDP echo server using the `mio` crate to multiplex reads and
writes.  This program uses edge-triggered events, which can
theoretically provide better performance than level-triggering by
reducing the overhead related to selection.  Handlers are expected to
perform as much I/O as possible until `WouldBlock` is indicated, at
which time `Poll::poll()` is called again and other file descriptors may
be handled.  Any mitigation of the edge-triggered starvation problem is
up to the application, and no such mitigation is demonstrated here.

- `echo-tokio.rs`:
A simple UDP echo server using Tokio to multiplex reads and writes.

- `echo-tokio-mpsc.rs`:
A simple UDP echo server using Tokio to multiplex reads and writes.
This is an alternate implementation that uses separate "reader" and
"writer" futures connected by an MPSC queue.

Futures and task notification
----------------------------------------

- `future-notify.rs`:
This small program demonstrates how futures can be manually scheduled
for polling by calling the notify() method on their task.

Understanding Mio
----------------------------------------

- `mio-empty.rs`:
This is an "empty" Mio example.  Mio is polled without having registered
for any events, so the `poll()` never returns.  This can be useful for
studying basic Mio behaviors that occur regardless of any registrations.
For example, when run via `strace` on Linux, we can see that Mio always
creates a pipe to accommodate non-system events sourced from user-space,
and then configures the underlying epoll to watch for read events on the
pipe.

- `mio-mixed.rs`:
This program demonstrates how a single Mio instance can be used to
receive both system events (e.g. file descriptor events) and non-system
events (e.g. events sourced on user-space threads other than the thread
running the Mio poll).  We listen for incoming UDP datagrams on port
2000, and also listen for events created by our timer thread every three
seconds.  Running this program on Linux via `strace` shows how Mio
notifies the polling thread of the non-system event by writing to a
pipe.

- `mio-pipe.rs`:
Demonstrate a possible bug where Mio uses a pipe write to notify of a
mio::Registration event which occurs while epoll_wait() is not
happening.  For more details, see: https://github.com/carllerche/mio/issues/785

Multiple sockets in Tokio
----------------------------------------

There are several ways to have Tokio manage multiple sockets.

These programs listen for incoming UDP datagrams on IPv4 localhost ports
2000 through 2009, and print a summary of each datagram to the standard
output.

- `tokio-multisocket-join.rs`:
This implementation works by creating ten futures, each processing data
on one socket, and combining them into a single composite future via
`join_all()`.  This composite future is provided to Tokio via
`Core::run()`.  This is a simple approach, but a possible downside is
that every future is polled whenever a single packet arrives on a
socket.  This is because all the futures run within a single task.
Because notifications happen at the task level, any notification
arranged in any of the futures will cause the main task to be notified.
It will poll the top-level `FromAll` future, which itself will poll each
of its children.

- `tokio-multisocket-spawn.rs`:
This implementation works by creating ten futures, each added to the
event loop within a distinct task via `Handle::spawn()`.  This spawning
is performed by a `UdpMultiServer` future which is passed to Tokio as
the main future via Core::run().  In contrast to
`tokio-multisocket-join.rs`, this approach avoids polling all futures
when only a single future needs to be polled.

- `tokio-multisocket-futuresunordered.rs`:
This implementation works by creating ten futures, each processing data
on one socket, and managing them with a `FuturesUnordered` stream, which
is provided to Tokio's `Core::run()` by way of the stream's `for_each()`
method.  `FuturesUnordered` has a very useful property that makes it
potentially more attractive (in this scenario) than a simple join.  A
`Join` future, when polled, will in turn poll all of its active futures,
even if only one future needs to be polled (i.e., only one future
arranged a notification event for the task which contains the `Join` and
its child futures).  In contrast, ["Futures managed by
`FuturesUnordered` will only be polled when they generate
notifications"](https://docs.rs/futures/0.1.18/futures/stream/futures_unordered/struct.FuturesUnordered.html).
This is accomplished by `FuturesUnordered::poll()` polling each future
with a distinct `NotifyHandle` thread-local, so it can perform
per-future notification discrimination and only poll the futures that
need to be polled.  When this program is run, you can observe that only
the correct future is polled.

Building
--------------------

The example programs can be built with `cargo build`.  The `select()`
and `epoll` examples may be built on suitable platforms if the `select`
and/or `epoll` feature flags are enabled.  For example:

```
cargo build --features=select,epoll
```

License
--------------------

This project is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   http://opensource.org/licenses/MIT)

at your option.  (The same license terms as Tokio itself.)
