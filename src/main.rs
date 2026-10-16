use anyhow::{bail, Context, Result};
use moodle_doc_converter::{convert_docx, render_moodle_text_file};
use std::{env, fs::File};

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.get(1).map(String::as_str) == Some("text") {
        if args.len() != 3 {
            bail!("usage: {} text INPUT.xml", args[0]);
        }
        print!("{}", render_moodle_text_file(&args[2])?);
        return Ok(());
    }
    if args.len() != 3 {
        bail!(
            "usage: {} INPUT.docx OUTPUT.xml\n       {} text INPUT.xml",
            args[0],
            args[0]
        );
    }

    let input = File::open(&args[1]).with_context(|| format!("opening {}", args[1]))?;
    let xml = convert_docx(input).context("converting DOCX to Moodle XML")?;
    std::fs::write(&args[2], xml).with_context(|| format!("writing {}", args[2]))?;
    Ok(())
}
