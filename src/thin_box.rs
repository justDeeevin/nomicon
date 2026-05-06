use std::{
    alloc::Layout,
    marker::Unsize,
    ptr::{self, DynMetadata, NonNull, Pointee},
};

pub struct ThinBox<Dyn: ?Sized + Pointee<Metadata = DynMetadata<Dyn>>>(NonNull<WithMeta<Dyn, ()>>);

struct WithMeta<Dyn: ?Sized + Pointee<Metadata = DynMetadata<Dyn>>, T: ?Sized> {
    vtable: DynMetadata<Dyn>,
    _value: T,
}

impl<Dyn: ?Sized + Pointee<Metadata = DynMetadata<Dyn>>> ThinBox<Dyn> {
    // supposedly impl Trait here could cause odd behavior
    pub fn new_unsize<T: Unsize<Dyn>>(value: T) -> Self {
        let vtable = ptr::metadata(&value as &Dyn);
        Self(
            NonNull::from(Box::leak(Box::new(WithMeta {
                vtable,
                _value: value,
            })))
            .cast(),
        )
    }

    /// This will deref to the trait object. The actual value is no longer accessible.
    #[allow(
        clippy::mut_from_ref,
        reason = "this is only used internally to reduce code re-use in deref impls"
    )]
    unsafe fn deref(&self, offset: usize) -> &mut Dyn {
        unsafe {
            let data_ptr = (self.0.as_ptr() as *mut u8).add(offset);
            &mut *ptr::from_raw_parts_mut(data_ptr, self.0.as_ref().vtable)
        }
    }

    fn layout_and_offset(&self) -> (Layout, usize) {
        let header = Layout::new::<DynMetadata<Dyn>>();
        // SAFETY: the pointer was provided by boxing a given value
        let value = unsafe { self.0.as_ref().vtable }.layout();
        let (layout, offset) = header.extend(value).unwrap();
        (layout.pad_to_align(), offset)
    }
}

impl<Dyn: ?Sized + Pointee<Metadata = DynMetadata<Dyn>>> std::ops::Deref for ThinBox<Dyn> {
    type Target = Dyn;

    fn deref(&self) -> &Self::Target {
        // SAFETY: the offset is provided by Layout::extend
        unsafe { self.deref(self.layout_and_offset().1) }
    }
}

impl<Dyn: ?Sized + Pointee<Metadata = DynMetadata<Dyn>>> std::ops::DerefMut for ThinBox<Dyn> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: the offset is provided by Layout::extend
        unsafe { self.deref(self.layout_and_offset().1) }
    }
}

impl<Dyn: ?Sized + Pointee<Metadata = DynMetadata<Dyn>>> Drop for ThinBox<Dyn> {
    fn drop(&mut self) {
        let (layout, offset) = self.layout_and_offset();
        unsafe {
            ptr::drop_in_place(self.deref(offset));
            std::alloc::dealloc(self.0.cast().as_ptr(), layout);
        }
    }
}
