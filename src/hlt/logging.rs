use std::fs::File;
use std::io::Write;
use std::fs::OpenOptions;

pub struct Logger(File);

impl Logger {
    pub fn new(user_id: usize) -> Logger {
        let file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(format!("log_{}.txt", user_id))
            .expect("Couldn't open file for logging!");
        Logger(file)
    }

    pub fn log(&mut self, message: &str) -> bool {
        self.0
            .write(message.as_bytes())
            .expect("Couldn't write to log!");
        self.0
            .write("\n".as_bytes())
            .expect("Couldn't write to log!");
        true
    }
}
