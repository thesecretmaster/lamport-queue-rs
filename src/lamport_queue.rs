use std::default::Default;
use std::ops::Deref;
use std::ptr;
use std::sync::{atomic, Arc};

pub struct LamportQueue<D: Default + Clone> {
    elements: Box<[D]>,
    head: usize,
    tail: usize,
    open: bool,
}

// Hoping that the Reader and Writer are effectively `newtype`
// and therefore 0 cost
pub struct LamportQueueReader<D: Default + Clone> {
    q: Arc<LamportQueue<D>>,
}

pub struct LamportQueueWriter<D: Default + Clone> {
    q: Arc<LamportQueue<D>>,
}

// Most of the methods on Reader and Writer are equally valid on either
// and simply read the underlying Queue, so I treat Reader/Writer
// as smart pointers and use Deref to make them callable.
impl<D: Default + Clone> LamportQueue<D> {
    pub fn new(capacity: usize) -> (LamportQueueReader<D>, LamportQueueWriter<D>) {
        let buffer = vec![D::default(); capacity];
        let q = Arc::new(Self {
            elements: buffer.into_boxed_slice(),
            head: 0,
            tail: 0,
            open: true,
        });
        (
            LamportQueueReader { q: Arc::clone(&q) },
            LamportQueueWriter { q: Arc::clone(&q) },
        )
    }

    pub fn len(&self) -> usize {
        unsafe { ptr::read_volatile(&self.tail) - ptr::read_volatile(&self.head) }
    }

    pub fn capacity(&self) -> usize {
        self.elements.len()
    }

    pub fn closed(&self) -> bool {
        unsafe { !ptr::read_volatile(&self.open) }
    }

    // This really should be &mut and should live in Reader or Writer,
    // because you should need to own either the reader or the writer
    // to close the queue.
    // I've decided to put it here instead and leave it as a shared
    // reference (`&`) rather than exclusive (`&mut`) is because it's
    // technically an OK thing to do, and it avoids duplicating the code.
    // Anyways, I don't really like this, but it's fine I guess :/
    pub fn close(&self) {
        unsafe { ptr::write_volatile(ptr::addr_of!(self.open) as *mut bool, false) }
    }
}

impl<D: Default + Clone> Deref for LamportQueueReader<D> {
    type Target = LamportQueue<D>;

    fn deref(&self) -> &Self::Target {
        &self.q
    }
}

impl<D: Default + Clone> Deref for LamportQueueWriter<D> {
    type Target = LamportQueue<D>;

    fn deref(&self) -> &Self::Target {
        &self.q
    }
}

// Actual push/pop implimentations
impl<D: Default + Clone> LamportQueueWriter<D> {
    pub fn push(&mut self, d: D) -> bool {
        // In theory, all reads to `self.tail` can be done without volatile, since
        // we can guarantee that we are the only writer to it. However, we only have
        // an immutable reference to `self.tail` and we use a volatile write to
        // mutate it to ensure the Writer sees our write, so I'm unsure how the
        // compiler would elide accesses across the volatile write.
        // 
        // So, I've left in a single volatile read to avoid the potential for compiler
        // access elision.
        let tail = unsafe { ptr::read_volatile(&self.tail) };

        // Ensure that the list is not full
        // It is not a problem if head returns an artifically low number because
        // the caller can simply retry the operation
        if (tail - unsafe { ptr::read_volatile(&self.head) }) == self.elements.len() {
            return false;
        }
        unsafe { ptr::write_volatile(ptr::addr_of!(self.elements[tail % self.elements.len()]) as *mut D, d) };

        // While rust's volatile guarentees that the compiler won't reorder memory accesses,
        // the fence is still necessary to avoid hardware reordering
        atomic::fence(atomic::Ordering::SeqCst);

        unsafe { ptr::write_volatile(ptr::addr_of!(self.tail) as *mut usize, tail + 1) };

        true
    }
}

impl<D: Default + Clone> LamportQueueReader<D> {
    pub fn pop(&mut self) -> Option<D> {
        // After this read, the value of `tail` will always be >= `self.tail`
        let tail = unsafe { ptr::read_volatile(&self.tail) };

        // In theory, all reads to `self.head` can be done without volatile, since
        // we can guarantee that we are the only writer to it. However, we only have
        // an immutable reference to `self.head` and we use a volatile write to
        // mutate it to ensure the Writer sees our write, so I'm unsure how the
        // compiler would elide accesses across the volatile write.
        // 
        // So, I've left in a single volatile read to avoid the potential for compiler
        // access elision.
        let head = unsafe { ptr::read_volatile(&self.head) };

        // `tail > head` & `self.tail >= tail` => `self.tail > head`
        // => It is safe to read one element off of the head
        if tail - head == 0 {
            return None;
        }
        let idx = head % self.elements.len();
        let e = unsafe { ptr::read_volatile(ptr::addr_of!(self.elements[idx])) };

        // While rust's volatile guarentees that the compiler won't reorder memory accesses,
        // the fence is still necessary to avoid hardware reordering
        atomic::fence(atomic::Ordering::SeqCst);

        unsafe { ptr::write_volatile(ptr::addr_of!(self.head) as *mut usize, head + 1) };

        Some(e)
    }
}
