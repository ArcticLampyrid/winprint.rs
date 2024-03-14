#![cfg(windows)]

mod bindings;
pub mod printer;
pub mod test_utils;
pub mod ticket;
mod utils;
#[cfg(test)]
mod tests {
    use ctor::ctor;

    #[ctor]
    fn setup() {
        env_logger::init();
    }
}
