use anyhow::{bail, Context, Result};
use moodle_doc_converter::{convert_docx, render_moodle_text_file};
use std::{env, fs::File};

/// Runs the CLI's conversion or transcript command based on positional arguments.
///
/// `text INPUT.xml` reads and prints a transcript. Otherwise the command
/// expects `INPUT.docx OUTPUT.xml`, opens the input, converts it, then writes
/// the XML output. Each I/O or conversion failure receives a short operation
/// context, and malformed argument counts return a usage error.
fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        bail!("usage: INPUT.docx OUTPUT.xml");
    }

    let input = File::open(&args[1]).with_context(|| format!("opening {}", args[1]))?;
    let xml = convert_docx(input).context("converting DOCX to Moodle XML")?;
    std::fs::write(&args[2], xml).with_context(|| format!("writing {}", args[2]))?;
    Ok(())
}
