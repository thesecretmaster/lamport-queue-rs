use rand::Rng;
use std::sync::atomic;
use std::sync::mpsc;

mod lamport_queue;
mod srsw_queue_verifier;

use lamport_queue::LamportQueue;
use srsw_queue_verifier::VerificationChecker;

const QUEUE_LEN: usize = 3;

fn main() {
    // Create the queue and prepare it for sharing
    let (mut reciever_handle, mut sender_handle) = LamportQueue::new(QUEUE_LEN);
    let (tx, rx) = mpsc::sync_channel(QUEUE_LEN);

    // Generate a list of random numbers to send down the queue
    let mut srsw_queue_verifier: VerificationChecker<usize> = VerificationChecker::new(100000, |_| rand::thread_rng().gen());
    let mut mpsc_verifier = srsw_queue_verifier.clone_send();

    atomic::compiler_fence(atomic::Ordering::SeqCst);

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

    atomic::compiler_fence(atomic::Ordering::SeqCst);

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
