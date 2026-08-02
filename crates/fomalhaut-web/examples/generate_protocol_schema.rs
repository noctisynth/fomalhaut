use std::{env, error::Error, fs};

use fomalhaut_web::protocol::schema_json_pretty;

fn main() -> Result<(), Box<dyn Error>> {
    let path = env::args_os().nth(1).ok_or("expected output path")?;
    fs::write(path, schema_json_pretty()?)?;
    Ok(())
}
