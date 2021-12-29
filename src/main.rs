//use std::alloc::{alloc, dealloc, Layout};
//use std::mem::align_of;
//use std::slice::from_raw_parts;
//
//fn new_heap_buffer<D, 'a>(capacity: usize) -> Box<&'a D> {
//    let buffer: *mut D = unsafe { Layout::from_size_align(capacity, align_of::<D>()).unwrap() };
//    let slice: &mut D = unsafe { from_raw_parts(buffer, capacity) };
//    Box::from_raw(slice)
//}
use std::default::Default;
use std::sync::atomic;
use std::thread;
use rand::Rng;
use std::sync::Arc;
use std::cell::UnsafeCell;
use std::ops::{Deref, DerefMut};

trait Queue<D> {
    fn new(capacity: usize) -> Self;
    fn push(&mut self, _: D) -> bool;
    fn pop(&mut self) -> Option<D>;
    fn closed(&self) -> bool;
    fn close(&mut self);
    fn capacity(&self) -> usize;
    fn len(&self) -> usize;
}

unsafe trait LockFreeQueue<D>: Queue<D> {
    fn new<E: Queue<D>>(capacity: usize) -> E {
        Queue::new(capacity)
    }

    fn push(&self, data: D) -> bool {
        let q = self as *const Self;
        let qm = q as *mut Self;
        unsafe { Queue::push(&mut *qm, data) }
    }


    fn pop(&mut self) -> Option<D> {
        let q = self as *const Self;
        let qm = q as *mut Self;
        unsafe { Queue::pop(&mut *qm) }
    }

    fn closed(&self) -> bool {
        Queue::closed(self)
    }

    fn close(&mut self);
    fn capacity(&self) -> usize;
    fn len(&self) -> usize;
}
struct LamportQueue<D: Default + Clone> {
    elements: Box<[D]>,
    head: usize,
    tail: usize,
    open: bool
}

impl<D: Default + Clone> Queue<D> for LamportQueue<D> {
    fn new(capacity: usize) -> LamportQueue<D> {
        let buffer = vec![D::default(); capacity];
        Self {
            elements: buffer.into_boxed_slice(),
            head: 0,
            tail: 0,
            open: true
        }
    }

    fn len(&self) -> usize {
        self.tail - self.head
    }

    fn capacity(&self) -> usize {
        self.elements.len()
    }

    fn close(&mut self) {
        self.open = false
    }

    fn closed(&self) -> bool {
        !self.open
    }

    fn push(&mut self, d: D) -> bool {
        if self.tail - self.head == ((*self.elements).len()) {
            return false;
        }
        self.elements[self.tail % self.elements.len()] = d;
        atomic::fence(atomic::Ordering::SeqCst);
        self.tail += 1;
        true
    }

    fn pop(&mut self) -> Option<D> {
        let tail = self.tail;
        if tail - self.head == 0 {
            return None;
        }
        let e = self.elements[self.head % self.elements.len()].clone();
        atomic::fence(atomic::Ordering::SeqCst);
        self.head += 1;
        Some(e)
    }
}

struct AtomicLamportQueue<D: Clone + Default> {
    v: UnsafeCell<LamportQueue<D>>
}

unsafe impl<T: Clone + Default> Sync for AtomicLamportQueue<T> {}

// This feels like it should be an impl DerefMut on Arc<AtomicLamportQueue<D>>, but
// I can't impliment DerefMut on Arc since it's not my type
fn mut_thru_arc<D: Clone + Default>(a: &mut Arc<AtomicLamportQueue<D>>) -> &mut LamportQueue<D> {
    unsafe { a.v.get().as_mut().unwrap() }
}

impl<D: Default + Clone> Deref for AtomicLamportQueue<D> {
    type Target = LamportQueue<D>;

    fn deref(&self) -> &Self::Target {
        unsafe { self.v.get().as_ref().unwrap() }
    }
}
impl<D: Default + Clone> DerefMut for AtomicLamportQueue<D> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.v.get().as_mut().unwrap() }
    }
}

impl<D: Clone + Default> AtomicLamportQueue<D> {
    fn new(capacity: usize) -> AtomicLamportQueue<D> {
        Self {
            v: UnsafeCell::new(LamportQueue::new(capacity))
        }
    }
}

const SIZE: usize = 10000; // Number of random numbers to use to test the queue

fn main() {
    // Create the queue and prepare it for sharing
    let queue: AtomicLamportQueue<u32> = AtomicLamportQueue::new(3);
    let shared_queue = Arc::new(queue);
    let mut sender_handle = Arc::clone(&shared_queue);
    let mut reciever_handle = Arc::clone(&shared_queue);

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
            while !mut_thru_arc(&mut sender_handle).push(*i) { }
        }
        // Close the loop to allow the reciever to terminate
        mut_thru_arc(&mut sender_handle).close();
        rng_list
    });

    let reciever = thread::spawn(move || {
        // Continually pop off the queue until it's closed and empty
        while !(reciever_handle.closed() && reciever_handle.len() == 0) {
            match mut_thru_arc(&mut reciever_handle).pop() {
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
