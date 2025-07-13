use crate::Cell;
use std::{marker::PhantomData, ops::Deref, ptr::NonNull};

struct RcInner<T> {
    count: Cell<usize>,
    data: T,
}

pub struct Rc<T> {
    ptr: NonNull<RcInner<T>>,
    _marker: PhantomData<RcInner<T>>,
}

impl<T> Rc<T> {
    pub fn new(data: T) -> Self {
        let inner = Box::new(RcInner {
            count: Cell::new(1),
            data,
        });

        Self {
            ptr: unsafe { NonNull::new_unchecked(Box::into_raw(inner)) },
            _marker: PhantomData,
        }
    }
}

impl<T> Deref for Rc<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &self.ptr.as_ref().data }
    }
}

impl<T> Clone for Rc<T> {
    fn clone(&self) -> Self {
        let inner = unsafe { self.ptr.as_ref() };
        inner.count.set(inner.count.get() + 1);

        Self {
            ptr: self.ptr,
            _marker: PhantomData,
        }
    }
}

impl<T> Drop for Rc<T> {
    fn drop(&mut self) {
        let inner = unsafe { self.ptr.as_mut() };
        if inner.count.get() == 1 {
            drop(unsafe { Box::from_raw(self.ptr.as_ptr()) });
        } else {
            inner.count.set(inner.count.get() - 1);
        }
    }
}
