use std::marker::PhantomPinned;
use std::pin::Pin;

#[derive(Debug)]
struct Test {
    a: String,
    b: *const String,
    _marker: PhantomPinned,
}

impl Test {
    fn new(txt: &str) -> Self {
        Test {
            a: String::from(txt),
            b: std::ptr::null(),
            _marker: PhantomPinned, // これは型を`!Unpin`にします。
        }
    }

    fn init(self: Pin<&mut Self>) {
        let self_ptr: *const String = &self.a;
        let this = unsafe { self.get_unchecked_mut() };
        this.b = self_ptr;
    }

    fn a(self: Pin<&Self>) -> &str {
        &self.get_ref().a
    }

    fn b(self: Pin<&Self>) -> &String {
        assert!(
            !self.b.is_null(),
            "Test::b called without Test::init being called first"
        );
        unsafe { &*(self.b) }
    }
}

// pub fn main() {
//     // test1を初期化する前に、test1の移動は安全です。
//     let mut test1 = Test::new("test1");
//     // `test`が再度、アクセスされることを避けるために、`test1`をどのように隠すか注意してください。
//     let mut test1 = unsafe { Pin::new_unchecked(&mut test1) };
//     Test::init(test1.as_mut());
//
//     let mut test2 = Test::new("test2");
//     let mut test2 = unsafe { Pin::new_unchecked(&mut test2) };
//     Test::init(test2.as_mut());
//
//     println!(
//         "a: {}, b: {}",
//         Test::a(test1.as_ref()),
//         Test::b(test1.as_ref())
//     );
//     println!(
//         "a: {}, b: {}",
//         Test::a(test2.as_ref()),
//         Test::b(test2.as_ref())
//     );
// }

// pub fn main() {
//     // test1を初期化する前に、test1の移動は安全です。
//     let mut test1 = Test::new("test1");
//     // `test`が再度、アクセスされることを避けるために、`test1`をどのように隠すか注意してください。
//     let mut test1 = unsafe { Pin::new_unchecked(&mut test1) };
//     Test::init(test1.as_mut());
//
//     let mut test2 = Test::new("test2");
//     let mut test2 = unsafe { Pin::new_unchecked(&mut test2) };
//     Test::init(test2.as_mut());
//
//     println!(
//         "a: {}, b: {}",
//         Test::a(test1.as_ref()),
//         Test::b(test1.as_ref())
//     );
//     std::mem::swap(test1.get_mut(), test2.get_mut());
//     println!(
//         "a: {}, b: {}",
//         Test::a(test2.as_ref()),
//         Test::b(test2.as_ref())
//     );
// }

fn main() {
    let mut test1 = Test::new("test1");
    let mut test1_pin = unsafe { Pin::new_unchecked(&mut test1) };
    Test::init(test1_pin.as_mut());

    std::mem::drop(test1_pin);
    println!(r#"test1.b points to "test1": {:?}..."#, test1.b);

    let mut test2 = Test::new("test2");
    std::mem::swap(&mut test1, &mut test2);
    println!("... and now it points nowhere: {:?}", test1.b);
}
