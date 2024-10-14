use std::fs::File;
use std::io::{BufReader, Read, Write};

use anyhow::Context;
use ihex::{BinaryReader, Reader};

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args();
    args.next().expect("executable in args");
    let ref input_path = args.next().context("Missing input file")?;
    let ref output_path = args.next().context("Missing output file")?;

    let input_file = File::open(input_path)
        .with_context(|| format!("Failed to read file: {input_path}"))?;
    let mut output_file = File::create(output_path)
        .with_context(|| format!("Failed to creat file: {output_path}"))?;
    let reader = Reader::new(BufReader::new(input_file));
    let mut binary_reader = BinaryReader::new(reader);
    let mut buf = [0; 64];
    loop {
        match binary_reader.read(&mut buf) {
            Ok(n) => {
                output_file.write_all(&buf[..n])
                    .with_context(|| format!("Failed to write to {output_path}"))?;
                if n < buf.len() {
                    break;
                }
            }
            Err(e) => panic!("Error: {}", e),
        }
    }

    Ok(())
}
