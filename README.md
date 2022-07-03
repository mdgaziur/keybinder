# Keybinder
![crates.io](https://img.shields.io/crates/v/keybinder.svg)

Wraps Keybinder in a safe way

# Example

```rust
use keybinder::KeyBinder;

fn main() {
    gtk::init().expect("Failed to init GTK");
    let data = String::from("some data");
    let mut keybinder = KeyBinder::<String>::new(true).expect("Keybinder is not supported");

    assert_eq!(keybinder.bind("<Shift>space", |key, data| {
        println!("key: {} , data: {}", key, data);
        gtk::main_quit();
    }, data), true);
    println!("Successfully bound keystring to handler");
    gtk::main();
}
```
