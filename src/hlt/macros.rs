#[macro_use]
mod macros {
    macro_rules! assert_unreachable (
    () => { panic!(format!("line {}", line!())) }
    );
}
