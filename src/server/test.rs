use core::ffi::c_void;
use core::marker::PhantomData;
use core::mem::transmute;
use core::ptr::null_mut;

/// ErasedFnPointer can either points to a free function or associated one that
/// `&mut self`
struct ErasedFnPointer<'a, T, Ret> {
    struct_pointer: *mut c_void,
    fp: *const (),
    // The `phantom_*` field is used so that the compiler won't complain about
    // unused generic parameter.
    phantom_sp: PhantomData<&'a ()>,
    phantom_fp: PhantomData<fn(T) -> Ret>,
}

impl<'a, T, Ret> Copy for ErasedFnPointer<'a, T, Ret> {}
impl<'a, T, Ret> Clone for ErasedFnPointer<'a, T, Ret> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, T, Ret> ErasedFnPointer<'a, T, Ret> {
    pub fn from_associated<S>(struct_pointer: &'a mut S, fp: fn(&mut S, T) -> Ret) -> Self {
        ErasedFnPointer {
            struct_pointer: struct_pointer as *mut _ as *mut c_void,
            fp: fp as *const (),
            phantom_sp: PhantomData,
            phantom_fp: PhantomData,
        }
    }

    pub fn from_free(fp: fn(T) -> Ret) -> ErasedFnPointer<'static, T, Ret> {
        ErasedFnPointer {
            struct_pointer: null_mut(),
            fp: fp as *const (),
            phantom_sp: PhantomData,
            phantom_fp: PhantomData,
        }
    }

    pub fn call(&self, param: T) -> Ret {
        if self.struct_pointer.is_null() {
            let fp = unsafe { transmute::<_, fn(T) -> Ret>(self.fp) };
            fp(param)
        } else {
            let fp = unsafe { transmute::<_, fn(*mut c_void, T) -> Ret>(self.fp) };
            fp(self.struct_pointer, param)
        }
    }
}

fn main() {
    let erased_ptr = ErasedFnPointer::from_free(|x| {
        println!("Hello, {}", x);
        x
    });
    erased_ptr.call(2333);

    println!(
        "size_of_val(erased_ptr) = {}",
        core::mem::size_of_val(&erased_ptr)
    );

    ErasedFnPointer::from_associated(&mut Test { x: 1 }, Test::f).call(1);

    let mut x = None;
    ErasedFnPointer::from_associated(&mut x, |x, param| {
        *x = Some(param);
        println!("{:#?}", x);
    })
    .call(1);
}

struct Test {
    x: i32,
}
impl Test {
    fn f(&mut self, y: i32) -> i32 {
        let z = self.x + y;
        println!("Hello from Test, {}", z);
        z
    }
}
