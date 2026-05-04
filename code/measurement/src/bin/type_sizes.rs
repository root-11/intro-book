//! Print sizes of common types and how many fit in a 64-byte cache line.
//! Used by §2 exercises 1 and 2.

const LINE: usize = 64;

fn main() {
    println!("{:<10} {:>5}  {:>10}", "type", "bytes", "per cache line");
    macro_rules! show {
        ($t:ty) => {{
            let s = std::mem::size_of::<$t>();
            println!("{:<10} {:>5}  {:>10}", stringify!($t), s, LINE / s);
        }}
    }
    show!(u8);
    show!(u16);
    show!(u32);
    show!(u64);
    show!(i32);
    show!(f32);
    show!(f64);
    show!(usize);
}
