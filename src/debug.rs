#[cfg(feature = "debug")]
macro_rules! debugln {
    ($( $args:expr ),*) => { println!( $( $args ),* ); }
}

#[cfg(not(feature = "debug"))]
macro_rules! debugln {
    ($( $args:expr ),*) => {}
}
