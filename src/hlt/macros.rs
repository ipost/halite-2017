#[macro_use]
#[allow(unused_macros)]
mod macros {
    macro_rules! assert_unreachable (
        () => { panic!(format!("line {}", line!())) }
        );

    macro_rules! in_360 (
        ($angle:expr) => (($angle + 360.0) % 360.0)
        );

    macro_rules! print_timing (
        ($code: block) => {{
            let pt_start_time = PreciseTime::now();
            let res = $code;
            Logger::new(0).log(&format!("  time at line {}: {}", line!(), pt_start_time.to(PreciseTime::now())));
            res
        }}
            );
}
