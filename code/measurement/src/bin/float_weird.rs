//! Float corner cases: NaN, infinity, sqrt(0), catastrophic cancellation.
//! Used by §2 exercises 4 and 5.

fn main() {
    println!("0.0 / 0.0       = {}", 0.0_f64 / 0.0_f64);
    println!("1.0 / 0.0       = {}", 1.0_f64 / 0.0_f64);
    println!("(0.0).sqrt()    = {}", (0.0_f64).sqrt());

    let nan = 0.0_f64 / 0.0_f64;
    println!("NaN != NaN      : {}", nan != nan);
    println!("NaN.is_nan()    : {}", nan.is_nan());

    let f32_diff = 1e10_f32 - (1e10_f32 - 1.0_f32);
    let f64_diff = 1e10_f64 - (1e10_f64 - 1.0_f64);
    println!();
    println!("Catastrophic cancellation:");
    println!("  f32: 1e10 - (1e10 - 1.0) = {}", f32_diff);
    println!("  f64: 1e10 - (1e10 - 1.0) = {}", f64_diff);

    // §2 exercise 7 stretch - show f32 epsilon
    println!();
    println!("f32::MAX        = {:e}", f32::MAX);
    println!("f32::EPSILON    = {:e}", f32::EPSILON);
    println!("f64::EPSILON    = {:e}", f64::EPSILON);
}
