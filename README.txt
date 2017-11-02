
Asynchronous I/O examples
======================================================================


Problems with echo-udp:
----------------------------------------

At least as of the latest tokio-core (25f30c9).

It may be a bit too simple, as it avoids some real-world situations:
1. Starvation
    - Continuous activity on one socket may prevent r/w messages on another socket.
    - Within the same socket, lots of incoming low-priority messages may
      prevent processing incoming high-priority messages.
2. No userspace buffering (i.e. relying completely on the in-kernel
buffers) means that higher-priority datagrams can't be processed.
    - e.g. One backlogged SCTP stream can prevent other streams from
      being received.
    - When kernel buffer fills, "shutdown" message may not be received/processed.
3. Flip-flop design:
    1. Not reflective of a real system that may need to process reads
    and writes at the same time.
    2. A blocked write prevents reads from happening.  (See starvation
    point 2.)

TODO: How many of these will be addressed by the toy server?

I looked at the irc crate which provides more of a real-world example,
but it uses separate threads for reading and writing.

Description of our toy servers
----------------------------------------

Better echo:
1. No flip-flop; inner loop allows both send and recv at any time.
    (Drop incoming if needed.)

Producer-Consumer:
1. Listen to localhost:2000
2. Relay packet to any interested consumers registered on localhost:2001
    consumers register by sending ASCII "1" and de-register by sending "0".

Implementations
----------------------------------------

1. select()
2. epoll()
3. mio
4. tokio

