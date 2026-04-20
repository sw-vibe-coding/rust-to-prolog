pub trait Foo {}

pub fn bad(_x: &dyn Foo) {}
