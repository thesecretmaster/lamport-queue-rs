use rand::Rng;
use std::sync::atomic;
use std::sync::mpsc;
use clap::{App, Arg};

mod lamport_queue;
mod srsw_queue_verifier;

use lamport_queue::LamportQueue;
use lamport_queue::{LamportQueueReader, LamportQueueWriter};
use srsw_queue_verifier::VerificationChecker;

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
    let srsw_queue_verifier: VerificationChecker<usize> = VerificationChecker::new(100000, |_| rand::thread_rng().gen());
    let mpsc_verifier = srsw_queue_verifier.clone_send();

    atomic::compiler_fence(atomic::Ordering::SeqCst);

    test_lamport(srsw_queue_verifier, sender_handle, reciever_handle);

    // This probably does nothing, but it's here just in case :P
    atomic::compiler_fence(atomic::Ordering::SeqCst);

    test_mpsc(mpsc_verifier, tx, rx);
}

fn test_mpsc(mut mpsc_verifier: VerificationChecker<usize>, tx: mpsc::SyncSender<usize>, rx: mpsc::Receiver<usize>) {
    mpsc_verifier.run_sender(move |sl| {
        for i in sl.into_iter() {
            // Continually retry until the push is sucessful
            while tx.send(*i).is_err() {}
        }
        // Automatically closes when it's dropped
    });

    mpsc_verifier.run_reciever(move |rl| {
        // Continually pop off the queue until it's closed and empty
        loop {
            match rx.recv() {
                Err(_) => break,
                Ok(i) => rl.push(i),
            }
        }
    });

    println!("MPSC:");
    mpsc_verifier.verify();
}

fn test_lamport(mut srsw_queue_verifier: VerificationChecker<usize>, mut sender_handle: LamportQueueWriter<usize>, mut reciever_handle: LamportQueueReader<usize>) {
    srsw_queue_verifier.run_sender(move |sl| {
        for i in sl.into_iter() {
            // Continually retry until the push is sucessful
            while !sender_handle.push(*i) {}
        }
        // Close the loop to allow the reciever to terminate
        sender_handle.close();
    });

    srsw_queue_verifier.run_reciever(move |rl| {
        // Continually pop off the queue until it's closed and empty
        while !(reciever_handle.closed() && reciever_handle.len() == 0) {
            match reciever_handle.pop() {
                None => (),
                Some(i) => rl.push(i),
            }
        }
    });

    println!("SRSW:");
    srsw_queue_verifier.verify();
}
