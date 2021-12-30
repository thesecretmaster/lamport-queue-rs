use rand::Rng;
use std::sync::atomic;
use std::sync::mpsc;
use closure::closure;

mod lamport_queue;
mod srsw_queue_verifier;

use lamport_queue::LamportQueue;
use srsw_queue_verifier::VerificationChecker;

fn main() {
    // Create the queue and prepare it for sharing
    let (reciever_handle, sender_handle) = LamportQueue::new(3);

    // Generate a list of random numbers to send down the queue
    let mut verifier: VerificationChecker<usize> = VerificationChecker::new(1000, |_| rand::thread_rng().gen());

    atomic::compiler_fence(atomic::Ordering::SeqCst);

    verifier.run_sender(closure!(move mut sender_handle, |sl| {
        for i in sl.into_iter() {
            // Continually retry until the push is sucessful
            while !sender_handle.push(*i) {}
        }
        // Close the loop to allow the reciever to terminate
        sender_handle.close();
    }));

    verifier.run_reciever(closure!(move mut reciever_handle, |rl| {
        // Continually pop off the queue until it's closed and empty
        while !(reciever_handle.closed() && reciever_handle.len() == 0) {
            match reciever_handle.pop() {
                None => (),
                Some(i) => rl.push(i),
            }
        }
    }));

    verifier.verify();
}
