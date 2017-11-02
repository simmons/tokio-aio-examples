// A simple UDP echo server using Tokio to multiplex reads and writes.
// This is an alternate implementation that uses separate "reader" and
// "writer" futures connected by an MPSC queue.

extern crate futures;
#[macro_use]
extern crate tokio_core;

use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use futures::{Async, Future, Poll};
use futures::Sink;
use futures::Stream;
use futures::sync::mpsc;
use tokio_core::net::UdpSocket;
use tokio_core::reactor::Core;

const MAX_MESSAGE_SIZE: usize = 1500;
const MAX_OUTGOING_MESSAGES: usize = 8;
const ECHO_PORT: u16 = 2000;

struct Message {
    buffer: Vec<u8>, // The contents of the message.
    addr: SocketAddr, // The original source address (and echo destination).
}

struct UdpReader<'a> {
    socket: &'a UdpSocket,
    tx: mpsc::Sender<Message>,
    message: Option<Message>,
    message_poll: bool,
}

impl<'a> UdpReader<'a> {
    fn new(socket: &UdpSocket, tx: mpsc::Sender<Message>) -> UdpReader {
        UdpReader {
            socket,
            tx,
            message: None,
            message_poll: false,
        }
    }
}

impl<'a> Future for UdpReader<'a> {
    type Item = ();
    type Error = io::Error;

    fn poll(&mut self) -> Poll<(), io::Error> {
        println!("Reader: poll()");

        if self.message_poll {
            // A previous poll() set the message_poll flag indicating that the MPSC queue needs to
            // be flushed, so flush it.
            match self.tx.poll_complete() {
                Ok(Async::Ready(())) => {
                    self.message_poll = false;
                }
                Ok(Async::NotReady) => {
                    return Ok(Async::NotReady);
                }
                Err(e) => {
                    panic!("Error flushing MPSC sink: {:?}", e);
                }
            }
        }

        // If a message was received on the previous poll(), begin sending it to the writer via the
        // MPSC queue.
        let message = self.message.take();
        if let Some(message) = message {
            match self.tx.start_send(message) {
                Ok(futures::AsyncSink::Ready) => {
                    println!("Reader: Message sent to the MPSC sink.");
                    // Flag that the next iteration of poll() should call poll_complete() on the
                    // sink, and arrange to be polled again as soon as possible.
                    self.message_poll = true;
                    futures::task::current().notify();
                    return Ok(Async::NotReady);
                }
                Ok(futures::AsyncSink::NotReady(m)) => {
                    println!("Reader: Message NOT sent to the MPSC sink -- we will try again later.");
                    self.message = Some(m);
                    return Ok(Async::NotReady);
                }
                Err(e) => {
                    panic!("Error sending to MPSC sink: {:?}", e);
                }
            }
        }

        // Read from the socket, if possible.
        // Note that try_nb! will return if recv_from() returns a WouldBlock error.
        let mut buffer = vec![0; MAX_MESSAGE_SIZE];
        let (nbytes, addr) = try_nb!(self.socket.recv_from(&mut buffer));
        println!("Reader: Message received.");

        // If this point is reached, then we were able to read a datagram.  Trim the buffer and
        // store the message.  It will be processed in the next poll().
        //
        // This is a bit different from the usual recommended method of trying to read as much as
        // possible in each poll() by looping on recv_from() until WouldBlock is indicated.
        // Instead, we only read (at most) one datagram per poll().  If the read was successful, we
        // ask the event loop to poll us again as soon as possible (in notify() below), then
        // return.  This way, the event loop could theoretically choose to run other tasks and
        // futures before calling us again, thus preventing our future from starving other tasks of
        // cycles.  (Google "edge-triggered starvation" for more on this.)
        buffer.truncate(nbytes);
        let message = Message { buffer, addr };
        self.message = Some(message);

        // Arrange to be polled again as soon as possible.
        futures::task::current().notify();

        return Ok(Async::NotReady);
    }
}

struct UdpWriter<'a> {
    socket: &'a UdpSocket,
    rx: mpsc::Receiver<Message>,
    message: Option<Message>,
}

impl<'a> UdpWriter<'a> {
    fn new(socket: &UdpSocket, rx: mpsc::Receiver<Message>) -> UdpWriter {
        UdpWriter {
            socket,
            rx,
            message: None,
        }
    }
}

impl<'a> Future for UdpWriter<'a> {
    type Item = ();
    type Error = io::Error;

    fn poll(&mut self) -> Poll<(), io::Error> {
        println!("Writer: poll()");

        // If a previous poll() received a new message from the MPSC queue, then try to send it.
        if let Some(ref message) = self.message {
            println!("Writer: Trying to send message...");
            // Note that try_nb! will return if send_to() indicates a WouldBlock error.
            try_nb!(self.socket.send_to(&message.buffer, &message.addr));
            drop(message);
            println!("Writer: Message sent.");
        }
        self.message = None;

        // Poll the MPSC queue.
        match self.rx.poll() {
            Ok(Async::Ready(Some(message))) => {
                println!("Writer: Message received from MPSC queue.");
                // If a message was received, store it in our state and arrange to be polled again
                // as soon as possible.  In the next poll() we will try to send the message.
                self.message = Some(message);
                futures::task::current().notify();
            }
            Ok(Async::Ready(None)) => {
                // The incoming stream has terminated, so our work here is done.
                return Ok(Async::Ready(()));
            }
            Ok(Async::NotReady) => {
                return Ok(Async::NotReady);
            }
            Err(e) => {
                panic!("error polling mpsc future: {:?}", e);
            }
        };

        return Ok(Async::NotReady);
    }
}

fn main() {
    let localhost = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));

    // Create the tokio event loop
    let mut core = Core::new().unwrap();

    // Open a UDP socket in non-blocking mode bound to IPv4 localhost port 2000.
    let socket = UdpSocket::bind(&SocketAddr::new(localhost, ECHO_PORT), &core.handle()).unwrap();

    // Create the reader and writer futures, and join them into a single composite future.
    // Arranging for the reader and writer futures to each run in their own separately-scheduled
    // task (via spawn()) is left as an exercise for the reader.
    let (tx, rx) = mpsc::channel(MAX_OUTGOING_MESSAGES);
    let reader = UdpReader::new(&socket, tx);
    let writer = UdpWriter::new(&socket, rx);
    let server = writer.join(reader);

    // Run the tokio event loop
    core.run(server).unwrap();
}
