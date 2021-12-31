use rand::Rng;
use std::sync::atomic;
use std::sync::mpsc;
use clap::{App, Arg};

mod lamport_queue;
mod spsc_queue_checker;

use lamport_queue::LamportQueue;
use lamport_queue::{LamportQueueReader, LamportQueueWriter};
use spsc_queue_checker::SPSCChecker;

fn main() {
    let matches = App::new("tsm's lamport queue")
                      .version("0.0.1")
                      .author("thesecretmaster <thesecretmaster@dvtk.me>")
                      .about("Simple single reader / singler writer lamport queue performance testing (vs mpsc::sync_channel)")
                      .arg(Arg::with_name("queue length")
                           .short("l")
                           .long("queue-length")
                           .value_name("QUEUE LENGTH")
                           .help("Sets the queue length (both lamport queues and mpsc::sync_channel have queue lengths)")
                           .takes_value(true)
                           .required(true)
                           .default_value("3"))
                      .get_matches();

    let queue_len: usize = clap::value_t!(matches.value_of("queue length"), usize).expect("Positive integer is required for queue length");

    println!("Running tests with queue length of {}", queue_len);

    // Create the queue and prepare it for sharing
    let (reciever_handle, sender_handle) = LamportQueue::new(queue_len);
    let (tx, rx) = mpsc::sync_channel(queue_len);

    // Generate a list of random numbers to send down the queue
    let spsc_queue_checker: SPSCChecker<usize> = SPSCChecker::new(100000, |_| rand::thread_rng().gen());
    let mpsc_checker = spsc_queue_checker.clone_send();

    atomic::compiler_fence(atomic::Ordering::SeqCst);

    test_lamport(spsc_queue_checker, sender_handle, reciever_handle);

    // This probably does nothing, but it's here just in case :P
    atomic::compiler_fence(atomic::Ordering::SeqCst);

    test_mpsc(mpsc_checker, tx, rx);
}

fn test_mpsc(mut mpsc_checker: SPSCChecker<usize>, tx: mpsc::SyncSender<usize>, rx: mpsc::Receiver<usize>) {
    mpsc_checker.run_reciever(move |rl| {
        // Continually pop off the queue until it's closed and empty
        loop {
            match rx.recv() {
                Err(_) => break,
                Ok(i) => rl.push(i),
            }
        }
    });

    mpsc_checker.run_sender(move |sl| {
        for i in sl.into_iter() {
            // Continually retry until the push is sucessful
            while tx.send(*i).is_err() {}
        }
        // Automatically closes when it's dropped
    });

    println!("MPSC:");
    mpsc_checker.check();
}

fn test_lamport(mut spsc_queue_checker: SPSCChecker<usize>, mut sender_handle: LamportQueueWriter<usize>, mut reciever_handle: LamportQueueReader<usize>) {
    spsc_queue_checker.run_reciever(move |rl| {
        // Continually pop off the queue until it's closed and empty
        while !(reciever_handle.closed() && reciever_handle.len() == 0) {
            match reciever_handle.pop() {
                None => (),
                Some(i) => rl.push(i),
            }
        }
    });

    spsc_queue_checker.run_sender(move |sl| {
        for i in sl.into_iter() {
            // Continually retry until the push is sucessful
            while !sender_handle.push(*i) {}
        }
        // Close the loop to allow the reciever to terminate
        sender_handle.close();
    });

    println!("SPSC:");
    spsc_queue_checker.check();
}
