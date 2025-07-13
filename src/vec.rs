use std::{
    alloc::{Layout, alloc, dealloc, handle_alloc_error, realloc},
    marker::PhantomData,
    mem::{forget, size_of},
    ops::Deref,
    ptr::{self, NonNull},
};

struct RawVec<T> {
    ptr: NonNull<T>,
    cap: usize,
}
unsafe impl<T: Send> Send for RawVec<T> {}
unsafe impl<T: Sync> Sync for RawVec<T> {}

impl<T> RawVec<T> {
    fn new() -> Self {
        let cap = if size_of::<T>() == 0 { usize::MAX } else { 0 };
        RawVec {
            ptr: NonNull::dangling(),
            cap,
        }
    }

    fn grow(&mut self) {
        // ZSTs set cap to usize::MAX, so growing indicates an overflow.
        assert!(size_of::<T>() != 0, "capacity overflow");

        let (new_cap, new_layout) = if self.cap == 0 {
            (1, Layout::array::<T>(1).unwrap())
        } else {
            // Cannot overflow because self.cap <= isize::MAX.
            let new_cap = 2 * self.cap;

            // Cannot panic. Layout::array returns Err if the number of bytes is > usize::MAX, and
            // old_layout.size() <= isize::MAX.
            let new_layout = Layout::array::<T>(new_cap).unwrap();
            (new_cap, new_layout)
        };

        assert!(
            new_layout.size() <= isize::MAX as usize,
            "allocation too large"
        );

        let new_ptr = if self.cap == 0 {
            unsafe { alloc(new_layout) }
        } else {
            let old_layout = Layout::array::<T>(self.cap).unwrap();
            let old_ptr = self.ptr.as_ptr() as *mut u8;
            unsafe { realloc(old_ptr, old_layout, new_layout.size()) }
        };

        self.ptr = match NonNull::new(new_ptr as *mut T) {
            Some(p) => p,
            None => handle_alloc_error(new_layout),
        };
        self.cap = new_cap;
    }
}

impl<T> Drop for RawVec<T> {
    fn drop(&mut self) {
        let elem_size = size_of::<T>();

        if self.cap != 0 && elem_size != 0 {
            let layout = Layout::array::<T>(self.cap).unwrap();
            unsafe {
                dealloc(self.ptr.as_ptr() as *mut u8, layout);
            }
        }
    }
}

pub struct Vec<T> {
    buf: RawVec<T>,
    len: usize,
}

impl<T> Vec<T> {
    pub fn new() -> Self {
        Vec {
            buf: RawVec::new(),
            len: 0,
        }
    }

    pub fn push(&mut self, value: T) {
        if self.len == self.cap() {
            self.buf.grow();
        }

        unsafe {
            ptr::write(self.ptr().add(self.len), value);
        }

        // Can't fail. Grow would panic first.
        self.len += 1;
    }

    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            None
        } else {
            self.len -= 1;
            unsafe { Some(ptr::read(self.ptr().add(self.len))) }
        }
    }

    pub fn insert(&mut self, index: usize, elem: T) {
        assert!(index <= self.len, "index out of bounds");
        if self.len == self.cap() {
            self.buf.grow()
        }

        unsafe {
            ptr::copy(
                self.ptr().add(index),
                self.ptr().add(index + 1),
                self.len - index,
            );
            ptr::write(self.ptr().add(index), elem);
        }

        self.len += 1;
    }

    pub fn remove(&mut self, index: usize) -> T {
        assert!(index < self.len, "index out of bounds");
        self.len -= 1;
        let result = unsafe { ptr::read(self.ptr().add(index)) };
        unsafe {
            ptr::copy(
                self.ptr().add(index + 1),
                self.ptr().add(index),
                self.len - index,
            );
        }
        result
    }

    pub fn drain(&mut self) -> Drain<T> {
        let iter = unsafe { RawValIter::new(self) };
        self.len = 0;

        Drain {
            iter,
            vec: PhantomData,
        }
    }

    fn ptr(&self) -> *mut T {
        self.buf.ptr.as_ptr()
    }

    fn cap(&self) -> usize {
        self.buf.cap
    }
}

impl<T> Default for Vec<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Drop for Vec<T> {
    fn drop(&mut self) {
        while self.pop().is_some() {}
    }
}

impl<T> Deref for Vec<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        unsafe { std::slice::from_raw_parts(self.ptr(), self.len) }
    }
}

struct RawValIter<T> {
    start: *const T,
    end: *const T,
}

impl<T> RawValIter<T> {
    unsafe fn new(slice: &[T]) -> Self {
        RawValIter {
            start: slice.as_ptr(),
            end: if size_of::<T>() == 0 {
                ((slice.as_ptr() as usize) + slice.len()) as *const _
            } else if slice.is_empty() {
                slice.as_ptr()
            } else {
                unsafe { slice.as_ptr().add(slice.len()) }
            },
        }
    }
}

impl<T> Iterator for RawValIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if std::ptr::eq(self.start, self.end) {
            None
        } else if size_of::<T>() == 0 {
            self.start = (self.start as usize + 1) as *const _;
            unsafe { Some(ptr::read(NonNull::<T>::dangling().as_ptr())) }
        } else {
            let old_ptr = self.start;
            self.start = unsafe { self.start.offset(1) };
            unsafe { Some(ptr::read(old_ptr)) }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let elem_size = size_of::<T>();
        let len =
            (self.end as usize - self.start as usize) / if elem_size == 0 { 1 } else { elem_size };
        (len, Some(len))
    }
}

impl<T> DoubleEndedIterator for RawValIter<T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if std::ptr::eq(self.start, self.end) {
            None
        } else if size_of::<T>() == 0 {
            self.end = (self.end as usize - 1) as *const _;
            unsafe { Some(ptr::read(NonNull::<T>::dangling().as_ptr())) }
        } else {
            self.end = unsafe { self.end.offset(-1) };
            unsafe { Some(ptr::read(self.end)) }
        }
    }
}

pub struct IntoIter<T> {
    _buf: RawVec<T>,
    iter: RawValIter<T>,
}

impl<T> IntoIterator for Vec<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        let iter = unsafe { RawValIter::new(&self) };
        let buf = unsafe { ptr::read(&self.buf) };
        forget(self);

        IntoIter { _buf: buf, iter }
    }
}

impl<T> Iterator for IntoIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<T> DoubleEndedIterator for IntoIter<T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.iter.next_back()
    }
}

impl<T> Drop for IntoIter<T> {
    fn drop(&mut self) {
        for _ in &mut *self {}
    }
}

pub struct Drain<'a, T: 'a> {
    vec: PhantomData<&'a mut Vec<T>>,
    iter: RawValIter<T>,
}

impl<T> Iterator for Drain<'_, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<T> DoubleEndedIterator for Drain<'_, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.iter.next_back()
    }
}

impl<T> Drop for Drain<'_, T> {
    fn drop(&mut self) {
        for _ in &mut *self {}
    }
}
