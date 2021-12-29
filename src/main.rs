use std::sync::atomic;
use std::thread;
use rand::Rng;

mod lamport_queue;

use lamport_queue::*;

const SIZE: usize = 10000; // Number of random numbers to use to test the queue

fn main() {
    // Create the queue and prepare it for sharing
    let (mut reciever_handle, mut sender_handle)  = LamportQueue::new(3);

    // Generate a list of random numbers to send down the queue to ensure it's
    // somewhat sane
    let mut rng_list = [0; SIZE];
    let mut recieved_numbers = Vec::with_capacity(SIZE);
    for i in &mut rng_list {
        *i = rand::thread_rng().gen()
    }

    atomic::compiler_fence(atomic::Ordering::SeqCst);

    let sender = thread::spawn(move || {
        for i in &rng_list {
            // Continually retry until the push is sucessful
            while !sender_handle.push(*i) { }
        }
        // Close the loop to allow the reciever to terminate
        sender_handle.close();
        rng_list
    });

    let reciever = thread::spawn(move || {
        // Continually pop off the queue until it's closed and empty
        while !(reciever_handle.closed() && reciever_handle.len() == 0) {
            match reciever_handle.pop() {
                None => (),
                Some(i) => {
                    recieved_numbers.push(i)
                }
            }
        }
        recieved_numbers
    });

    let rng_list = sender.join().unwrap();
    let recieved_numbers = reciever.join().unwrap();

    // Correctness checks
    let same_length = rng_list.len() == recieved_numbers.len();
    let mut numbers_match = true;
    for i in 0..SIZE {
        if rng_list[i] != recieved_numbers[i] {
            numbers_match = false;
            break;
        }
    }
    println!("Count is correct? {}", same_length);
    println!("Numbers match up? {}", numbers_match);
}
