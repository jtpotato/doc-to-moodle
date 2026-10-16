# moodle-doc-converter

Small Rust program that converts a Microsoft Word `.docx` question manuscript
into a Moodle question-bank XML file.

The converter currently produces Moodle `multichoice` questions only. It
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

Print a readable transcript of an existing Moodle XML file:

```sh
cargo run --release -- text output.xml
```

The `text` subcommand is primarily for machines but also human readable. It prints questions and answers with appropriate formatting for a terminal.

Build and test the project:

```sh
cargo test
cargo build --release
```

## Authoring format

### Recommended explicit format

Put each question in its own section. A section begins with a marker, followed
by one or more prompt paragraphs, the literal `[Answers]` token, and one
answer per paragraph:

```text
[MCQ id=climate-1]
Which process removes carbon dioxide from the atmosphere?
The question may include several paragraphs and images before [Answers].
[Answers]
Photosynthesis (*)
Combustion
Evaporation
_
```

`id=` is optional. `[MCQ climate-1]` is equivalent to
`[MCQ id=climate-1]`. The marker's ID becomes the Moodle question name.

### Accepted question markers

The parser recognizes:

- `[MCQ id=climate-1]`
- `[MCQ climate-1]`
- GIFT-like `::question-id::`

Marker matching is applied to the trimmed paragraph text. A paragraph before
the first marker is ignored. A manuscript must contain at least one marker.

### Prompt and answer rules

- `[Answers]` is the unambiguous boundary between the prompt and choices and
  is strongly recommended for every new manuscript.
- Each answer must be its own paragraph.
- Append `(*)` to every correct answer. The marker is removed from the
  exported answer text.
- Answers without `(*)` receive zero credit.
- `~%0%` may be placed at the start of an answer to document an explicit
  zero-credit GIFT-style option. The token is removed from the exported text.
- A paragraph containing only `_` is ignored, as are empty paragraphs.
- Avoid starting an answer with `[MCQ` or `::`; those strings are question
  markers and begin a new question.

When `[Answers]` is absent, the legacy heuristic is used. Paragraphs remain
part of the prompt until the most recent prompt paragraph ends in `?` or
`answers)`, unless a paragraph looks like an answer (for example, it ends in
`(*)`). Images also keep a paragraph in the prompt. This compatibility mode
is less predictable than `[Answers]` and should not be used for new documents.

At least one answer is required for each question. A question with no answers,
or a document with no recognized markers, is rejected with an error.

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

Every parsed question is emitted as:

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
  marks, and transcript tests.

## Limitations and compatibility notes

- Only multiple-choice questions are generated; short answer, matching,
  true/false, and other Moodle types are not inferred.
- Only JPEG and PNG image payloads can be cropped. Other embedded image
  formats are not supported by the dependency configuration.
- The parser is paragraph-oriented. List numbering, tables, headers,
  footers, comments, tracked changes, and Word fields are not part of the
  supported authoring model.
- Moodle import behavior can still depend on the Moodle version and question
  bank settings. Inspect the generated XML or use the `text` subcommand before
  importing a large bank.
