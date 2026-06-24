trait Speakable {
    fn speak(&self);
}

struct Dog;

impl Speakable for Dog {
    fn speak(&self) {}
}
