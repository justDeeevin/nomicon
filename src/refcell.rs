use crate::Cell;
use std::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
};

#[derive(Clone, Copy)]
enum RefState {
    Shared(usize),
    Exclusive,
    None,
}

pub struct RefCell<T> {
    value: UnsafeCell<T>,
    state: Cell<RefState>,
}

impl<T> RefCell<T> {
    pub fn new(value: T) -> Self {
        Self {
            value: UnsafeCell::new(value),
            state: Cell::new(RefState::None),
        }
    }

    pub fn borrow(&self) -> Option<Ref<'_, T>> {
        match self.state.get() {
            RefState::None => {
                self.state.set(RefState::Shared(1));
                Some(Ref { cell: self })
            }
            RefState::Shared(count) => {
                self.state.set(RefState::Shared(count + 1));
                Some(Ref { cell: self })
            }
            RefState::Exclusive => None,
        }
    }

    pub fn borrow_mut(&self) -> Option<RefMut<'_, T>> {
        match self.state.get() {
            RefState::None => {
                self.state.set(RefState::Exclusive);
                Some(RefMut { cell: self })
            }
            _ => None,
        }
    }
}

pub struct Ref<'a, T> {
    cell: &'a RefCell<T>,
}

impl<T> Deref for Ref<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.cell.value.get() }
    }
}

impl<T> Drop for Ref<'_, T> {
    fn drop(&mut self) {
        let state = self.cell.state.get();
        match state {
            RefState::Shared(count) => {
                self.cell.state.set(RefState::Shared(count - 1));
            }
            RefState::Exclusive => {
                self.cell.state.set(RefState::None);
            }
            RefState::None => unreachable!(),
        }
    }
}

pub struct RefMut<'a, T> {
    cell: &'a RefCell<T>,
}

impl<T> Deref for RefMut<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.cell.value.get() }
    }
}

impl<T> DerefMut for RefMut<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.cell.value.get() }
    }
}

impl<T> Drop for RefMut<'_, T> {
    fn drop(&mut self) {
        self.cell.state.set(RefState::None);
    }
}
