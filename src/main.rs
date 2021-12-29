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

trait ImmutableQueue<D> {
    fn new(capacity: usize) -> Self;
    fn push(&self, _: D) -> bool;
    fn pop(&self) -> Option<D>;
    fn closed(&self) -> bool;
    fn close(&self);
    fn capacity(&self) -> usize;
    fn len(&self) -> usize;
}

struct LamportQueue<D: Default + Clone + Sized> {
    elements: Box<[D]>,
    head: usize,
    tail: usize,
    open: bool
}

impl<D: Default + Clone + Sized> Queue<D> for LamportQueue<D> {
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

struct AtomicLamportQueue<D: Clone + Default + Sized> {
    v: UnsafeCell<LamportQueue<D>>
}



fn mut_thru_arc<D: Sized + Clone + Default>(a: &mut Arc<AtomicLamportQueue<D>>) -> &mut LamportQueue<D> {
    unsafe { a.v.get().as_mut().unwrap() }
}

impl<D: Default + Sized + Clone> Deref for AtomicLamportQueue<D> {
    type Target = LamportQueue<D>;

    fn deref(&self) -> &Self::Target {
        unsafe { self.v.get().as_ref().unwrap() }
    }
}
impl<D: Default + Sized + Clone> DerefMut for AtomicLamportQueue<D> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.v.get().as_mut().unwrap() }
    }
}

impl<D: Clone + Default + Sized> AtomicLamportQueue<D> {
    fn new(capacity: usize) -> AtomicLamportQueue<D> {
        Self {
            v: UnsafeCell::new(LamportQueue::new(capacity))
        }
    }
}

unsafe impl<T: Clone + Default + Sized> Sync for AtomicLamportQueue<T> {}

const SIZE: usize = 10000;

fn main() {
    let q: AtomicLamportQueue<u32> = AtomicLamportQueue::new(3);
    let shared_q = Arc::new(q);
    let mut sender_handle = Arc::clone(&shared_q);
    let mut reciever_handle = Arc::clone(&shared_q);

    let mut rng_list = [0; SIZE];
    let mut recieved_numbers = Vec::with_capacity(SIZE);
    for i in &mut rng_list {
        *i = rand::thread_rng().gen()
    }
    atomic::compiler_fence(atomic::Ordering::SeqCst);
    let sender = thread::spawn(move || {
        for i in &rng_list {
            loop {
                if mut_thru_arc(&mut sender_handle).push(*i) {
                    break
                }
            }
        }
        mut_thru_arc(&mut sender_handle).close();
    });
    let reciever = thread::spawn(move || {
        loop {
            match mut_thru_arc(&mut reciever_handle).pop() {
                None => (),
                Some(i) => {
                    recieved_numbers.push(i)
                }
            }
            if reciever_handle.closed() && reciever_handle.len() == 0 {
                break
            }
        }
    });
    sender.join().unwrap();
    reciever.join().unwrap();
}
