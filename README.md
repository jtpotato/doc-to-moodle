# moodle-doc-converter

Small Rust program that converts a Microsoft Word `.docx` question manuscript
into a Moodle question-bank XML file.

The converter produces Moodle `multichoice` and `matching` questions. It
supports formatted text and embedded JPEG/PNG images, including cropping.

## Requirements

- Rust and Cargo (the project uses the Rust 2021 edition).
- A DOCX file containing the supported authoring format.
- Moodle access for importing the generated XML.

The project has no runtime configuration file and does not need a running
Moodle instance.

## Quick start

Convert a manuscript:

```sh
cargo run --release -- input.docx output.xml
```

The output path is overwritten if it already exists. Errors are reported with
the operation that failed, such as opening the input, reading the DOCX
package, converting it, or writing the XML.

Build the project:

```sh
cargo build --release
```

## Authoring format

Follow [Format Guide.md](Format%20Guide.md) for question markers, answer
formatting, and scoring. Each question uses a `::...::` marker followed by its
prompt, an `[Answers]` paragraph, and one answer per paragraph. Questions and
answers are read in document order. A question without answers, or a document
without question markers, is rejected.

### Formatting and images in Word

Formatting is read from Word runs:

- bold runs become `<strong>...</strong>`;
- italic runs become `<i>...</i>`;
- a run can contain both styles, producing nested HTML;
- tabs become literal tab characters;
- non-breaking spaces become ordinary spaces.

Images are retained inline at their position in the paragraph. The converter
reads image relationships that target `word/media/*`, supports JPEG and PNG
image data, and emits Moodle `<file>` elements alongside `<img>` references.
The image's Word drawing dimensions are converted from EMUs using 96-DPI
pixel sizing (9,525 EMUs per pixel), rounded, and clamped to at least one
pixel. If Word specifies a crop, the crop is applied to the embedded pixels
before they are base64-encoded into the Moodle XML. Repeated references to the
same media filename share one `<file>` payload.

## Moodle output

Multiple-choice questions are emitted as:

```xml
<question type="multichoice">
```

The name is XML-escaped. Prompt and answer text is HTML-escaped inside
CDATA-wrapped Moodle `<text>` elements. Answers are shuffled by Moodle.

The `<single>` field is `true` only when exactly one answer is marked correct.
With multiple correct answers it is `false`, and the positive fractions are
split evenly:

- one correct answer: `100`;
- two correct answers: `50` each;
- three correct answers: `33.3333333333` each;
- no correct answers: all answers receive `0`.

Images in prompt or answer HTML use `@@PLUGINFILE@@/<filename>`, and the
corresponding base64 data is included in the relevant question text's
`<file>` element. The XML is UTF-8 and wrapped in a `<quiz>` root element.

## Library API

The crate exposes the following useful entry points:

- `convert_docx(reader)`: reads any `Read + Seek` DOCX stream and returns
  Moodle XML.
- `parse_questions(paragraphs)`: turns the intermediate paragraph model into
  validated `Question` values.
- `render_moodle_text_file(path)` and `render_moodle_text(xml)`: produce the
  readable diagnostic transcript.
- `Inline`, `Paragraph`, `Answer`, and `Question`: the public intermediate
  model used to build or inspect questions in Rust code.

Matching rows use `label -> value`; each row becomes a Moodle matching pair,
with the label as the subquestion and the value as its matching answer. The
parser identifies matching questions from the `->` answer format; the marker
remains the question name.

The internal renderer is intentionally kept separate from the public API.
`moodle_xml_for_test` exists only as a small test helper for exercising XML
rendering without constructing a DOCX archive.

## Source map

- `src/main.rs`: command-line argument parsing, file I/O, and user-facing
  error context.
- `src/lib.rs`: public library exports and the DOCX-to-Moodle pipeline.
- `src/docx.rs`: DOCX ZIP entries, Word XML paragraphs/runs, styles, image
  relationships, EMU dimensions, and physical image cropping.
- `src/gift.rs`: question markers, prompt/answer boundary detection, answer
  correctness markers, and the intermediate data model.
- `src/moodle.rs`: Moodle XML, HTML/XML escaping, image references/files, and
  mark fractions.
- `src/text.rs`: XML transcript rendering and ANSI formatting for diagnostics.
- `tests/converter.rs`: end-to-end conversion plus focused parser, formatting,
  and marks tests.

## Limitations and compatibility notes

- Only multiple-choice and matching questions are generated; short answer,
  true/false, and other Moodle types are not inferred.
- Only JPEG and PNG image payloads can be cropped. Other embedded image
  formats are not supported by the dependency configuration.
- The parser is paragraph-oriented. List numbering, tables, headers,
  footers, comments, tracked changes, and Word fields are not part of the
  supported authoring model.
- Moodle import behavior can still depend on the Moodle version and question
  bank settings. Inspect the generated XML or use the `text` subcommand before
  importing a large bank.
