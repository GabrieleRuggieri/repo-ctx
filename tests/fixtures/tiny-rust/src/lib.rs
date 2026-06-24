pub struct Greeter;

impl Greeter {
    pub fn greet(&self) -> String {
        format_greeting("world")
    }
}

pub fn format_greeting(name: &str) -> String {
    format!("Hello, {name}")
}
