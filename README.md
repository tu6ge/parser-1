# PHP-Parser

A handwritten fault-tolerant, recursive-descent parser for PXP (the PHP superset) written in Rust.

> **Warning** - this is still alpha software and the public API is still subject to change. Please use at your own risk.

---

## Usage

Add `pxp-parser` in your `Cargo.toml`'s `dependencies` section

```toml
[dependencies]
pxp-parser = { git = "https://github.com/php-rust-tools/pxp-parser" }
```

or use `cargo add`

```sh
cargo add pxp-parser --git https://github.com/php-rust-tools/pxp-parser
```

## Differences

PXP is a strict superset of PHP so everything that is supported by PHP is also supported by PXP. Regardless of this, PXP is still able to implement a range of additional syntactial features on top of PHP's "vanilla" syntax.

* Short match expressions
* Multi-line match arms
* Local type aliases
* Range expressions `1..5` (exclusive), `1..=5` (inclusive), `1..` (endless)
* Auto-capturing multi-line closures
* Use-able type aliases (todo)
* Generics, including (but not limited to) `array<K, V>`, `Closure<(int, int), int>`  (todo)

The list above is just a short list of the current plans for PXP. As ideas and concepts are finalised, the list will be updated.

### Example

```rust
use std::io::Result;

use pxp_parser::parser;

const CODE: &str = r#"<?php

final class User {
    public function __construct(
        public readonly string $name,
        public readonly string $email,
        public readonly string $password,
    ) {
    }
}
"#;

fn main() -> Result<()> {
    match parser::parse(CODE) {
        Ok(ast) => {
            println!("{:#?}", ast);
        }
        Err(err) => {
            println!("{}", err.report(CODE, None, true, false)?);

            println!("parsed so far: {:#?}", err.partial);
        }
    }

    Ok(())
}
```

## License

Licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

## Credits

* [Ryan Chandler](https://github.com/ryangjchandler)
* [All contributors](https://github.com/ryangjchandler/php-parser-rs/graphs/contributors)
