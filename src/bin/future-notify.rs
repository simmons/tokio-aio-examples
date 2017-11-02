// This small program demonstrates how futures can be manually scheduled
// for polling by calling the notify() method on their task.

extern crate futures;

use std::thread;
use std::time::Instant;
use futures::{Async, Future, Poll, task};

struct MyFuture {
    count: usize,
    start_time: Instant
}

impl MyFuture {
    fn new() -> MyFuture {
        MyFuture { count: 0, start_time: Instant::now() }
    }
}

impl Future for MyFuture {
    type Item = ();
    type Error = ();
    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.count += 1;
        println!("future poll() count={} time={:?}", self.count, self.start_time.elapsed());
        if self.count < 10 {
            // Arrange to be polled again one second in the future by passing this future's task
            // handle to a thread which sleeps then notifies the task.
            let task = task::current();
            thread::spawn(move || {
                thread::sleep(std::time::Duration::from_secs(1));
                task.notify();
            });
            Ok(Async::NotReady)
        } else {
            println!("Future complete.");
            Ok(Async::Ready(()))
        }
    }
}

fn main() {
    let future = MyFuture::new();
    future.wait().unwrap();
}
