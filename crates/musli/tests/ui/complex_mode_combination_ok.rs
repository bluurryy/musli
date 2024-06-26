use musli::{Decode, Encode};

enum Packed {}

#[derive(Encode, Decode)]
#[musli(name_all = "name")]
#[musli(mode = Packed, encode_only, packed, name_all = "index")]
struct Person<'a> {
    name: &'a str,
    age: u32,
}

#[derive(Encode, Decode)]
#[musli(mode = Packed, encode_only, packed)]
enum Name<'a> {
    Full(&'a str),
    Given(&'a str),
}

fn main() {
}
