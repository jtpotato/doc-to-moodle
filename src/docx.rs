use anyhow::{bail, Context, Result};
use image::{DynamicImage, GenericImageView, ImageFormat};
use quick_xml::{events::Event, Reader};
use std::{
    collections::HashMap,
    io::{Cursor, Read, Seek},
};
use zip::ZipArchive;

use crate::gift::{Inline, Paragraph};

/// Resolved embedded image data and its display dimensions.
#[derive(Debug, Clone, PartialEq)]
pub struct Image {
    pub bytes: Vec<u8>,
    pub width_px: u32,
    pub height_px: u32,
}

/// The intermediate contents extracted from a DOCX package.
pub struct Document {
    pub paragraphs: Vec<Paragraph>,
    pub images: HashMap<String, Image>,
}

#[derive(Default)]
struct RunStyle {
    bold: bool,
    italic: bool,
}

#[derive(Default)]
struct RawImage {
    relationship_id: Option<String>,
    width_emu: u64,
    height_emu: u64,
    crop: Crop,
}

#[derive(Debug, Clone, Copy, Default)]
struct Crop {
    left: u32,
    top: u32,
    right: u32,
    bottom: u32,
}

impl Document {
    /// Reads a seekable DOCX ZIP archive and resolves its supported content.
    ///
    /// This includes Word paragraphs, run-level bold/italic state, image
    /// relationships, display dimensions, and Word crop rectangles.
    pub fn read<R: Read + Seek>(input: R) -> Result<Self> {
        let mut archive = ZipArchive::new(input).context("invalid DOCX zip archive")?;
        let xml = read_entry(&mut archive, "word/document.xml")?;
        let relationships = read_relationships(&mut archive)?;
        let (paragraphs, raw_images) = parse_document(&xml)?;

        let mut images = HashMap::new();
        let mut resolved_paragraphs = Vec::new();
        for mut paragraph in paragraphs {
            let mut inlines = Vec::new();
            for inline in paragraph.inlines.drain(..) {
                match inline {
                    Inline::Image(raw) => {
                        let raw_image = raw_images.get(&raw).with_context(|| {
                            format!("missing image data for relationship {raw}")
                        })?;
                        let target = relationships
                            .get(&raw_image.relationship_id.clone().unwrap_or_default())
                            .with_context(|| format!("missing relationship {raw}"))?;
                        let filename = target.rsplit('/').next().unwrap_or(target).to_string();
                        let source_bytes = read_entry(&mut archive, &format!("word/{target}"))?;
                        let bytes = crop_image(&source_bytes, raw_image.crop)
                            .with_context(|| format!("cropping image {filename}"))?;
                        images.entry(filename.clone()).or_insert(Image {
                            bytes,
                            width_px: emu_to_px(raw_image.width_emu),
                            height_px: emu_to_px(raw_image.height_emu),
                        });
                        inlines.push(Inline::Image(filename));
                    }
                    other => inlines.push(other),
                }
            }
            paragraph.inlines = inlines;
            if !paragraph.is_empty() {
                resolved_paragraphs.push(paragraph);
            }
        }
        Ok(Self {
            paragraphs: resolved_paragraphs,
            images,
        })
    }
}

fn emu_to_px(emu: u64) -> u32 {
    ((emu as f64 / 9_525.0).round() as u32).max(1)
}

fn read_entry<R: Read + Seek>(archive: &mut ZipArchive<R>, name: &str) -> Result<Vec<u8>> {
    let mut entry = archive
        .by_name(name)
        .with_context(|| format!("missing DOCX entry {name}"))?;
    let mut bytes = Vec::new();
    entry.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn read_relationships<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
) -> Result<HashMap<String, String>> {
    let data = read_entry(archive, "word/_rels/document.xml.rels")?;
    let mut reader = Reader::from_reader(Cursor::new(data));
    let mut buffer = Vec::new();
    let mut relationships = HashMap::new();
    loop {
        match reader.read_event_into(&mut buffer)? {
            Event::Empty(event) | Event::Start(event)
                if event.name().as_ref() == b"Relationship" =>
            {
                let mut id = None;
                let mut target = None;
                for attribute in event.attributes().flatten() {
                    match attribute.key.as_ref() {
                        b"Id" => id = Some(String::from_utf8_lossy(&attribute.value).into_owned()),
                        b"Target" => {
                            target = Some(String::from_utf8_lossy(&attribute.value).into_owned())
                        }
                        _ => {}
                    }
                }
                if let (Some(id), Some(target)) = (id, target) {
                    if target.starts_with("media/") {
                        relationships.insert(id, target);
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    Ok(relationships)
}

fn parse_document(data: &[u8]) -> Result<(Vec<Paragraph>, HashMap<String, RawImage>)> {
    let mut reader = Reader::from_reader(Cursor::new(data));
    let mut buffer = Vec::new();
    let mut paragraphs = Vec::new();
    let mut raw_images: HashMap<String, RawImage> = HashMap::new();
    let mut paragraph = None;
    let mut style = RunStyle::default();
    let mut in_run = false;
    let mut image_counter = 0;
    let mut active_image: Option<String> = None;

    loop {
        match reader.read_event_into(&mut buffer)? {
            Event::Start(event) if event.name().as_ref() == b"w:p" => {
                paragraph = Some(Paragraph::default())
            }
            Event::End(event) if event.name().as_ref() == b"w:p" => {
                if let Some(value) = paragraph.take() {
                    if !value.is_empty() {
                        paragraphs.push(value);
                    }
                }
            }
            Event::Start(event) if event.name().as_ref() == b"w:r" => {
                in_run = true;
                style = RunStyle::default();
            }
            Event::Start(event) if event.name().as_ref() == b"w:drawing" => {
                let key = format!("image-{image_counter}");
                image_counter += 1;
                raw_images.insert(key.clone(), RawImage::default());
                active_image = Some(key.clone());
                if let Some(value) = paragraph.as_mut() {
                    value.inlines.push(Inline::Image(key));
                }
            }
            Event::End(event) if event.name().as_ref() == b"w:drawing" => {
                active_image = None;
            }
            Event::End(event) if event.name().as_ref() == b"w:r" => {
                in_run = false;
            }
            Event::Text(event) if paragraph.is_some() => {
                let text = event.unescape()?.replace('\u{a0}', " ");
                if let Some(value) = paragraph.as_mut() {
                    value
                        .inlines
                        .push(Inline::text(text, style.bold, style.italic));
                }
            }
            Event::Empty(event) if event.name().as_ref() == b"wp:extent" => {
                let image = active_image
                    .as_ref()
                    .and_then(|key| raw_images.get_mut(key));
                if let Some(image) = image {
                    image.width_emu = attribute_u64(&event, b"cx");
                    image.height_emu = attribute_u64(&event, b"cy");
                }
            }
            Event::Empty(event) if event.name().as_ref() == b"a:srcRect" => {
                let image = active_image
                    .as_ref()
                    .and_then(|key| raw_images.get_mut(key));
                if let Some(image) = image {
                    image.crop = Crop {
                        left: attribute_u32(&event, b"l"),
                        top: attribute_u32(&event, b"t"),
                        right: attribute_u32(&event, b"r"),
                        bottom: attribute_u32(&event, b"b"),
                    };
                }
            }
            Event::Empty(event) if event.name().as_ref() == b"a:blip" => {
                let relationship_id = event.attributes().flatten().find_map(|attribute| {
                    (attribute.key.as_ref() == b"r:embed")
                        .then(|| String::from_utf8_lossy(&attribute.value).into_owned())
                });
                if let Some(image) = active_image
                    .as_ref()
                    .and_then(|key| raw_images.get_mut(key))
                {
                    image.relationship_id = relationship_id;
                }
            }
            Event::Empty(event) if event.name().as_ref() == b"w:tab" => {
                if let Some(value) = paragraph.as_mut() {
                    value
                        .inlines
                        .push(Inline::text("\t", style.bold, style.italic));
                }
            }
            Event::Empty(event) if in_run => {
                update_style(&mut style, event.name().as_ref(), event.attributes());
            }
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    if paragraph.is_some() {
        bail!("malformed DOCX: unclosed paragraph");
    }
    Ok((paragraphs, raw_images))
}

fn update_style(
    style: &mut RunStyle,
    name: &[u8],
    attributes: quick_xml::events::attributes::Attributes<'_>,
) {
    if name != b"w:b" && name != b"w:i" {
        return;
    }
    let enabled = attributes
        .flatten()
        .find(|attribute| attribute.key.as_ref() == b"w:val")
        .map(|attribute| attribute.value.as_ref() != b"0" && attribute.value.as_ref() != b"false")
        .unwrap_or(true);
    if name == b"w:b" {
        style.bold = enabled;
    } else {
        style.italic = enabled;
    }
}

fn attribute_u64(event: &quick_xml::events::BytesStart<'_>, key: &[u8]) -> u64 {
    event
        .attributes()
        .flatten()
        .find_map(|attribute| {
            (attribute.key.as_ref() == key).then(|| {
                String::from_utf8_lossy(&attribute.value)
                    .parse()
                    .unwrap_or(0)
            })
        })
        .unwrap_or(0)
}

fn attribute_u32(event: &quick_xml::events::BytesStart<'_>, key: &[u8]) -> u32 {
    attribute_u64(event, key).min(100_000) as u32
}

fn crop_image(bytes: &[u8], crop: Crop) -> Result<Vec<u8>> {
    if crop.left == 0 && crop.top == 0 && crop.right == 0 && crop.bottom == 0 {
        return Ok(bytes.to_vec());
    }

    let format = image::guess_format(bytes).context("unknown image format")?;
    let image = image::load_from_memory_with_format(bytes, format)?;
    let (width, height) = image.dimensions();
    let left = width * crop.left / 100_000;
    let top = height * crop.top / 100_000;
    let right = width.saturating_sub(width * crop.right / 100_000);
    let bottom = height.saturating_sub(height * crop.bottom / 100_000);
    let crop_width = right.saturating_sub(left).max(1);
    let crop_height = bottom.saturating_sub(top).max(1);
    let cropped = image.crop_imm(left, top, crop_width, crop_height);

    let mut output = Cursor::new(Vec::new());
    let output_format = match format {
        image::ImageFormat::Jpeg => ImageFormat::Jpeg,
        image::ImageFormat::Png => ImageFormat::Png,
        other => bail!("unsupported cropped image format {other:?}"),
    };
    DynamicImage::write_to(&cropped, &mut output, output_format)?;
    Ok(output.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crops_pixels_using_docx_percentages() {
        let source = DynamicImage::new_rgb8(100, 80);
        let mut encoded = Cursor::new(Vec::new());
        source.write_to(&mut encoded, ImageFormat::Png).unwrap();
        let cropped = crop_image(
            encoded.get_ref(),
            Crop {
                left: 10_000,
                top: 25_000,
                right: 20_000,
                bottom: 25_000,
            },
        )
        .unwrap();
        let result = image::load_from_memory(&cropped).unwrap();
        assert_eq!(result.dimensions(), (70, 40));
    }
}
