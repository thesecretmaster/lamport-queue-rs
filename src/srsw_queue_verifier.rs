use std::thread;
use std::thread::JoinHandle;
use std::mem;
use std::time;

// A potential refactor here is to use an Either<A, B> type to store send_list / send_handle
// but since this isn't perf critical and I don't know how I'd name those fields, I'm leaving
// it as is.
pub struct VerificationChecker<T> {
    send_list_size: usize,
    // These lists are options because they can be lent out to the sender/reciever
    // threads, but only once each. Once they've been lent, they're None
    send_list: Option<Box<[T]>>, // A list of random numbers to send
    recieve_list: Option<Vec<T>>, // A location to deposit random numbers once recieved
    sender_handle: Option<JoinHandle<Box<[T]>>>,
    reciever_handle: Option<JoinHandle<Vec<T>>>,
    start_time: Option<time::Instant>,
}

impl<T: 'static + Default + Copy + PartialEq + Send> VerificationChecker<T> {
    pub fn new<F: FnMut(&mut T)>(send_list_size: usize, f: F) -> Self {
        let mut v: Box<[T]> = vec![T::default(); send_list_size].into_boxed_slice();
        v.iter_mut().for_each(f);
        Self {
            send_list_size,
            send_list: Some(v),
            recieve_list: Some(Vec::with_capacity(send_list_size)),
            sender_handle: None,
            reciever_handle: None,
            start_time: None,
        }
    }

    pub fn clone_send(&self) -> Self {
        Self {
            send_list_size: self.send_list_size,
            send_list: self.send_list.clone(),
            recieve_list: Some(Vec::with_capacity(self.send_list_size)),
            sender_handle: None,
            reciever_handle: None,
            start_time: None,
        }
    }

    pub fn verify(&mut self) -> bool {
        let send_list = self.sender_handle.take().expect("Cannot verify if a sender thread hasn't been started").join().unwrap();
        let recieve_list = self.reciever_handle.take().expect("Cannot verify if a reciever thread hasn't been started").join().unwrap();
        let end_time = self.start_time.expect("Nothing to time, nothing has been started").elapsed();
        let same_length = send_list.len() == recieve_list.len();
        let mut numbers_match = true;
        for i in 0..self.send_list_size {
            if send_list[i] != recieve_list[i] {
                numbers_match = false;
                break;
            }
        }
        println!("Count is correct? {}", same_length);
        println!("Numbers match up? {}", numbers_match);
        println!("Elapsed time: {}ms", end_time.as_millis());
        same_length && numbers_match
    }

    pub fn run_sender<F: 'static + FnOnce(&Box<[T]>) + Send>(&mut self, f: F) {
        let mut sl = None;
        mem::swap(&mut self.send_list, &mut sl);
        let sl = sl.expect("Only one sender can be started for each verifier");
        self.start_time = Some(time::Instant::now());
        self.sender_handle = Some(thread::spawn(move || {
            f(&sl);
            sl
        }));
    }

    pub fn run_reciever<F: 'static + FnOnce(&mut Vec<T>) + Send>(&mut self, f: F) {
        let mut rl = None;
        mem::swap(&mut self.recieve_list, &mut rl);
        let mut rl = rl.expect("Only one reciever can be started for each verifier");
        self.reciever_handle = Some(thread::spawn(move || {
            f(&mut rl);
            rl
        }));
    }
}
