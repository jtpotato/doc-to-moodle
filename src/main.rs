use anyhow::{bail, Context, Result};
use moodle_doc_converter::convert_docx_with_preview;
use std::{env, fs::File, io::IsTerminal};

/// Runs the CLI's conversion or transcript command based on positional arguments.
///
/// `text INPUT.xml` reads and prints a transcript. Otherwise the command
/// expects `INPUT.docx OUTPUT.xml`, opens the input, converts it, then writes
/// the XML output. Each I/O or conversion failure receives a short operation
/// context, and malformed argument counts return a usage error.
fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        bail!("usage: INPUT.docx OUTPUT.xml");
    }

    let input = File::open(&args[1]).with_context(|| format!("opening {}", args[1]))?;
    let color = std::io::stdout().is_terminal();
    let (xml, preview) =
        convert_docx_with_preview(input, color).context("converting DOCX to Moodle XML")?;
    std::fs::write(&args[2], xml).with_context(|| format!("writing {}", args[2]))?;
    print!("{preview}");
    Ok(())
}
