use schemars::schema_for;

use pxp_parser::parser::ast::Program;

fn main() {
    let schema = schema_for!(Program);
    println!("{}", serde_json::to_string_pretty(&schema).unwrap());
}
