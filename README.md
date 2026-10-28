# DOCX to Moodle XML

A client-side Next.js app that converts Word question documents into Moodle
question-bank XML. The converter runs in a Web Worker in the browser; documents
and generated XML are not sent to a server.

## Development

Requires Bun 1.4.4 and a recent browser with Web Workers, `createImageBitmap`,
and `OffscreenCanvas` support.

```sh
bun install --frozen-lockfile
bun run dev
```

Create a production build and run it locally with:

```sh
bun run build
bun run start
```

## Deploy to Vercel

Import the repository and use the repository root as the project root. Vercel
will detect the Next.js project from `package.json` and use the standard Next.js
build. The repository pins Bun in `vercel.json`.

## Supported authoring format

- Question markers: `[MCQ id=question-id]`, `[MCQ question-id]`,
  `::question-id::`, and the legacy `2026_D..._Q...` form.
- `[Answers]` starts the answer choices. Without it, the converter uses the
  question-mark and answer-marker heuristic.
- Put each answer in its own paragraph. Moodle GIFT answer markers are supported:
  `=` for a fully correct answer, `~` for an incorrect answer, and `~%50%` (or
  another percentage) for a partially correct answer. Percentages are preserved
  in the preview and Moodle XML. The `(*)` marker remains supported; when used
  without GIFT weights, its correct answers share 100% equally.
- Bold and italic Word runs, tabs, embedded JPEG/PNG images, display dimensions,
  and Word image cropping are preserved.
- At least one question marker and one answer per question are required.

The parser is paragraph-oriented. Tables, list numbering, headers, footers,
tracked changes, comments, and other Moodle question types are not converted.
The upload UI accepts DOCX files up to 50 MB.

## Project structure

- `src/app/`: Next.js App Router pages and styles.
- `src/components/ui/`: shadcn/ui components.
- `src/lib/converter.ts`: ZIP and Word XML parsing, question recognition,
  image extraction/cropping, and Moodle XML rendering.
- `src/workers/converter.worker.ts`: runs conversion off the UI thread.
