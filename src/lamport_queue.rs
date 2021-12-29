use std::default::Default;
use std::sync::{Arc, atomic};
use std::ops::Deref;
use std::ptr;

pub struct LamportQueue<D: Default + Clone> {
    elements: Box<[D]>,
    head: usize,
    tail: usize,
    open: bool
}

pub struct LamportQueueReader<D: Default + Clone> {
    q: Arc<LamportQueue<D>>
}

pub struct LamportQueueWriter<D: Default + Clone> {
    q: Arc<LamportQueue<D>>
}

pub trait LamportQueueAccessor<D: Default + Clone> {
    fn as_queue(&self) -> &LamportQueue<D>;

    fn len(&self) -> usize {
        self.as_queue().tail - self.as_queue().head
    }

    fn capacity(&self) -> usize {
        self.as_queue().elements.len()
    }

    fn close(&mut self) {
        unsafe { ptr::write_volatile(ptr::addr_of!(self.as_queue().open) as *mut bool, false) }
    }

    fn closed(&self) -> bool {
        !self.as_queue().open
    }
}

impl<D: Default + Clone> LamportQueueAccessor<D> for LamportQueueReader<D> {
    fn as_queue(&self) -> &LamportQueue<D> {
        self.q.deref()
    }
}

impl<D: Default + Clone> LamportQueueAccessor<D> for LamportQueueWriter<D> {
    fn as_queue(&self) -> &LamportQueue<D> {
        self.q.deref()
    }
}

impl<D: Default + Clone> LamportQueue<D> {
    pub fn new(capacity: usize) -> (LamportQueueReader<D>, LamportQueueWriter<D>) {
        let buffer = vec![D::default(); capacity];
        let q = Arc::new(Self {
            elements: buffer.into_boxed_slice(),
            head: 0,
            tail: 0,
            open: true
        });
        (LamportQueueReader { q: Arc::clone(&q) }, LamportQueueWriter { q: Arc::clone(&q) })
    }

}

impl<D: Default + Clone> LamportQueueWriter<D> {
    pub fn push(&mut self, d: D) -> bool {
        if unsafe { ptr::read_volatile(&self.q.tail) - ptr::read_volatile(&self.q.head) == ((*self.q.elements).len()) } {
            return false;
        }
        let data_location = ptr::addr_of!(self.q.elements[self.q.tail % self.q.elements.len()]) as *mut D;
        unsafe { ptr::write_volatile(data_location, d) };
        atomic::fence(atomic::Ordering::SeqCst);
        let initial_tail = unsafe { ptr::read_volatile(&self.q.tail) };
        unsafe { ptr::write_volatile(ptr::addr_of!(self.q.tail) as *mut usize, initial_tail + 1) };
        true
    }

}

impl<D: Default + Clone> LamportQueueReader<D> {
    pub fn pop(&mut self) -> Option<D> {
        let tail = unsafe { ptr::read_volatile(&self.q.tail) };
        if tail - unsafe { ptr::read_volatile(&self.q.head) } == 0 {
            return None;
        }
        let e = self.q.elements[self.q.head % self.q.elements.len()].clone();
        atomic::fence(atomic::Ordering::SeqCst);
        let initial_head = unsafe { ptr::read_volatile(&self.q.head) };
        unsafe { ptr::write_volatile(ptr::addr_of!(self.q.head) as *mut usize, initial_head + 1) };
        Some(e)
    }
}
