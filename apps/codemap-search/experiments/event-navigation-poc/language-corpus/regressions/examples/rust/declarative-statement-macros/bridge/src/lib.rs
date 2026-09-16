use core::ptr::NonNull;

pub struct Moving<T>(NonNull<T>);
impl<T> Moving<T> {
    pub unsafe fn from_value(value: &mut core::mem::MaybeUninit<T>) -> Self {
        Moving(NonNull::from(value).cast::<T>())
    }
    pub unsafe fn read(self) -> T {
        self.0.as_ptr().read()
    }
}
#[macro_export]
macro_rules! relocate {
    ($value:ident) => {
        let mut $value = ::core::mem::MaybeUninit::new($value);
        let $value = unsafe { $crate::Moving::from_value(&mut $value) };
    };
}

pub struct Layer<T>(pub T);
pub fn wrap<T>(value: T) -> Layer<T> { Layer(value) }

#[macro_export]
macro_rules! twice {
    ($value:ident) => {
        let $value = $crate::wrap($crate::wrap($value));
    };
}
