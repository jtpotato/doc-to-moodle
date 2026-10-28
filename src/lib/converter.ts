import { strFromU8, unzipSync } from "fflate";
import { XMLParser, XMLValidator } from "fast-xml-parser";
import { bytesToBase64, imageBytesToDataUrl } from "@/lib/image-data";

export type TextInline = { type: "text"; value: string; bold: boolean; italic: boolean };
export type ImageInline = { type: "image"; name: string; src?: string };
export type Inline = TextInline | ImageInline;
export type Answer = { correct: boolean; content: Inline[]; fraction?: number };
export type Question = { name: string; prompt: Inline[][]; answers: Answer[] };
export type ConversionResult = { questions: Question[]; xml: string };

type XmlNode = Record<string, unknown>;
type Crop = { left: number; top: number; right: number; bottom: number };
type RawImage = { relationshipId?: string; widthEmu: number; heightEmu: number; crop: Crop };
type EmbeddedImage = { bytes: Uint8Array; widthPx: number; heightPx: number; dataUrl: string };
type Paragraph = { inlines: Inline[] };

const parser = new XMLParser({
  preserveOrder: true,
  ignoreAttributes: false,
  attributeNamePrefix: "@_",
  trimValues: false,
  processEntities: true,
});

/** Converts a DOCX byte buffer into preview data and Moodle XML in the browser. */
export async function convertDocx(input: ArrayBuffer): Promise<ConversionResult> {
  let archive: Record<string, Uint8Array>;
  try {
    archive = unzipSync(new Uint8Array(input));
  } catch (error) {
    throw new Error(`Invalid DOCX ZIP archive: ${error instanceof Error ? error.message : String(error)}`);
  }

  const documentXml = readEntry(archive, "word/document.xml");
  const relationshipsXml = readEntry(archive, "word/_rels/document.xml.rels");
  const relationships = readRelationships(relationshipsXml);
  const rawImages = new Map<string, RawImage>();
  const paragraphs = parseDocument(documentXml, rawImages);
  const images = new Map<string, EmbeddedImage>();

  for (const paragraph of paragraphs) {
    for (let index = 0; index < paragraph.inlines.length; index++) {
      const inline = paragraph.inlines[index];
      if (inline.type !== "image") continue;
      const raw = rawImages.get(inline.name);
      if (!raw) throw new Error(`Missing image data for relationship ${inline.name}`);
      const target = raw.relationshipId ? relationships.get(raw.relationshipId) : undefined;
      if (!target) throw new Error(`Missing relationship ${inline.name}`);
      const filename = target.split("/").at(-1) || target;
      const source = archive[`word/${target}`];
      if (!source) throw new Error(`Missing DOCX entry word/${target}`);

      if (!images.has(filename)) {
        let bytes = source;
        try {
          bytes = await cropImage(source, raw.crop);
        } catch (error) {
          throw new Error(`Cropping image ${filename} failed: ${error instanceof Error ? error.message : String(error)}`);
        }
        images.set(filename, {
          bytes,
          widthPx: emuToPx(raw.widthEmu),
          heightPx: emuToPx(raw.heightEmu),
          dataUrl: imageBytesToDataUrl(bytes, imageMime(bytes)),
        });
      }
      paragraph.inlines[index] = { type: "image", name: filename, src: images.get(filename)!.dataUrl };
    }
  }

  const questions = parseQuestions(paragraphs);
  return { questions, xml: renderMoodleXml(questions, images) };
}

function readEntry(archive: Record<string, Uint8Array>, path: string): string {
  const bytes = archive[path];
  if (!bytes) throw new Error(`Missing DOCX entry ${path}`);
  const xml = strFromU8(bytes);
  const valid = XMLValidator.validate(xml);
  if (valid !== true) throw new Error(`Malformed XML in ${path}: ${valid.err.msg}`);
  return xml;
}

function parseXml(xml: string): XmlNode[] {
  return parser.parse(xml) as XmlNode[];
}

function elementOf(node: XmlNode): [string, unknown] | undefined {
  return Object.entries(node).find(([key]) => key !== ":@" && key !== "#text");
}

function childrenOf(node: XmlNode): XmlNode[] {
  const element = elementOf(node);
  return element && Array.isArray(element[1]) ? element[1] as XmlNode[] : [];
}

function attributesOf(node: XmlNode): Record<string, string> {
  const attrs = node[":@"];
  return attrs && typeof attrs === "object" ? attrs as Record<string, string> : {};
}

function attribute(node: XmlNode, name: string): string | undefined {
  const attributes = attributesOf(node);
  return attributes[`@_${name}`] ?? Object.entries(attributes).find(([key]) => key.endsWith(`:${name}`))?.[1];
}

function textOf(nodes: XmlNode[]): string {
  let text = "";
  for (const node of nodes) {
    if (typeof node["#text"] === "string") text += node["#text"];
    text += textOf(childrenOf(node));
  }
  return text;
}

function visit(nodes: XmlNode[], tag: string, callback: (node: XmlNode) => void): void {
  for (const node of nodes) {
    const element = elementOf(node);
    if (!element) continue;
    if (element[0] === tag) callback(node);
    visit(childrenOf(node), tag, callback);
  }
}

function readRelationships(xml: string): Map<string, string> {
  const relationships = new Map<string, string>();
  visit(parseXml(xml), "Relationship", node => {
    const id = attribute(node, "Id");
    const target = attribute(node, "Target");
    if (id && target?.startsWith("media/")) relationships.set(id, target);
  });
  return relationships;
}

function parseDocument(xml: string, images: Map<string, RawImage>): Paragraph[] {
  const paragraphs: Paragraph[] = [];
  let imageCounter = 0;
  const walk = (nodes: XmlNode[], inlines: Inline[], style: { bold: boolean; italic: boolean }): void => {
    for (const node of nodes) {
      const element = elementOf(node);
      if (!element) continue;
      const [tag, value] = element;
      const children = Array.isArray(value) ? value as XmlNode[] : [];

      if (tag === "w:r") {
        const runStyle = { bold: false, italic: false };
        const properties = children.find(child => elementOf(child)?.[0] === "w:rPr");
        if (properties) {
          for (const property of childrenOf(properties)) {
            const propertyTag = elementOf(property)?.[0];
            if (propertyTag !== "w:b" && propertyTag !== "w:i") continue;
            const enabled = attribute(property, "val");
            const on = enabled !== "0" && enabled !== "false";
            if (propertyTag === "w:b") runStyle.bold = on;
            else runStyle.italic = on;
          }
        }
        walk(children.filter(child => elementOf(child)?.[0] !== "w:rPr"), inlines, runStyle);
      } else if (tag === "w:t") {
        inlines.push({ type: "text", value: textOf(children).replace(/\u00a0/g, " "), ...style });
      } else if (tag === "w:tab") {
        inlines.push({ type: "text", value: "\t", ...style });
      } else if (tag === "w:drawing") {
        const key = `image-${imageCounter++}`;
        const raw: RawImage = { widthEmu: 0, heightEmu: 0, crop: { left: 0, top: 0, right: 0, bottom: 0 } };
        visit(children, "wp:extent", item => {
          raw.widthEmu = parseNumber(attribute(item, "cx"));
          raw.heightEmu = parseNumber(attribute(item, "cy"));
        });
        visit(children, "a:srcRect", item => {
          raw.crop = {
            left: boundedCrop(attribute(item, "l")),
            top: boundedCrop(attribute(item, "t")),
            right: boundedCrop(attribute(item, "r")),
            bottom: boundedCrop(attribute(item, "b")),
          };
        });
        visit(children, "a:blip", item => { raw.relationshipId = attribute(item, "embed"); });
        images.set(key, raw);
        inlines.push({ type: "image", name: key });
      } else {
        walk(children, inlines, style);
      }
    }
  };

  const collect = (nodes: XmlNode[]): void => {
    for (const node of nodes) {
      const tag = elementOf(node)?.[0];
      if (tag === "w:p") {
        const paragraph: Paragraph = { inlines: [] };
        walk(childrenOf(node), paragraph.inlines, { bold: false, italic: false });
        if (!isParagraphEmpty(paragraph)) paragraphs.push(paragraph);
      } else {
        collect(childrenOf(node));
      }
    }
  };

  collect(parseXml(xml));
  return paragraphs;
}

function parseQuestions(paragraphs: Paragraph[]): Question[] {
  const questions: Question[] = [];
  let current: Question | undefined;
  let answersStarted = false;
  const finish = () => {
    if (!current) return;
    if (current.answers.length === 0) throw new Error(`Question ${current.name} has no answers`);
    questions.push(current);
    current = undefined;
  };

  for (const paragraph of paragraphs) {
    const text = plainText(paragraph);
    const marker = questionMarker(text);
    if (marker !== undefined) {
      finish();
      current = { name: marker, prompt: [], answers: [] };
      answersStarted = false;
      continue;
    }
    if (!current) continue;
    if (text.trim() === "[Answers]") { answersStarted = true; continue; }
    if (text.trim() === "_" || (text.trim() === "" && isParagraphEmpty(paragraph))) continue;
    if (answersStarted) {
      const answer = parseAnswer(paragraph);
      if (answer) current.answers.push(answer);
      continue;
    }

    const explicitlyMarked = text.trim().endsWith("(*)") || /^[=~]/.test(text.trim());
    const previousPrompt = current.prompt.at(-1);
    const previousText = previousPrompt ? previousPrompt.filter((inline): inline is TextInline => inline.type === "text").map(inline => inline.value).join("") : "";
    const promptFinished = previousText.trim().endsWith("?") || previousText.trim().endsWith("answers)");
    const containsImage = paragraph.inlines.some(inline => inline.type === "image");
    if (current.answers.length === 0 && !explicitlyMarked && (!promptFinished || containsImage)) {
      current.prompt.push(paragraph.inlines);
    } else {
      const answer = parseAnswer(paragraph);
      if (answer) current.answers.push(answer);
      else if (current.answers.length === 0) current.prompt.push(paragraph.inlines);
    }
  }
  finish();
  if (questions.length === 0) throw new Error("No question markers found; add [MCQ id=...] or ::id:: markers");
  return questions;
}

function questionMarker(value: string): string | undefined {
  const text = value.trim();
  if (text.startsWith("[MCQ") && text.endsWith("]")) return text.slice(4, -1).trim().replace(/^id=/, "").trim();
  if (text.startsWith("2026_D") && text.includes("_Q")) return text;
  if (text.startsWith("::") && text.endsWith("::")) {
    const id = text.slice(2, -2).trim();
    if (id) return id;
  }
  return undefined;
}

function parseAnswer(paragraph: Paragraph): Answer | undefined {
  const text = plainText(paragraph);
  const trimmed = text.trimStart();
  const giftMarker = trimmed.match(/^([=~])(?:%(-?\d+(?:\.\d+)?)%)?/);
  const legacyCorrect = text.trim().endsWith("(*)");
  const fraction = giftMarker
    ? (giftMarker[2] === undefined ? (giftMarker[1] === "=" ? 100 : 0) : Number(giftMarker[2]))
    : undefined;
  const correct = giftMarker ? (fraction ?? 0) > 0 : legacyCorrect;
  const content = paragraph.inlines.map(inline => ({ ...inline }));
  if (giftMarker) {
    removeTextPrefix(content, giftMarker[0]);
  } else if (legacyCorrect) {
    const last = content.at(-1);
    if (last?.type === "text") {
      const index = last.value.lastIndexOf("(*)");
      if (index >= 0) last.value = last.value.slice(0, index);
    }
  }
  return content.length ? { correct, content, fraction } : undefined;
}

function removeTextPrefix(content: Inline[], prefix: string): void {
  const firstText = content.find(inline => inline.type === "text");
  if (firstText?.type === "text") {
    const leadingWhitespace = firstText.value.match(/^\s*/)?.[0] ?? "";
    const markerIndex = leadingWhitespace.length;
    if (firstText.value.startsWith(prefix, markerIndex)) {
      firstText.value = firstText.value.slice(0, markerIndex) + firstText.value.slice(markerIndex + prefix.length);
    }
  }
}

export function answerFractions(answers: Answer[]): number[] {
  const hasGiftWeights = answers.some(answer => answer.fraction !== undefined);
  if (hasGiftWeights) return answers.map(answer => answer.fraction ?? 0);
  const correctCount = answers.filter(answer => answer.correct).length;
  const inferred = correctCount === 0 ? 0 : 100 / correctCount;
  return answers.map(answer => answer.correct ? inferred : 0);
}

function renderMoodleXml(questions: Question[], images: Map<string, EmbeddedImage>): string {
  let output = `<?xml version="1.0" encoding="UTF-8"?>\n<quiz>\n`;
  for (const question of questions) {
    const fractions = answerFractions(question.answers);
    const correctCount = fractions.filter(fraction => fraction > 0).length;
    output += `  <question type="multichoice">\n`;
    output += `    <name><text>${xmlEscape(question.name)}</text></name>\n`;
    output += `    <questiontext format="html"><text><![CDATA[${question.prompt.map(paragraph => renderInlines(paragraph, images)).join("<br/>")}]]></text>`;
    for (const filename of imageNames(question.prompt)) {
      const image = images.get(filename);
      if (image) output += `<file name="${xmlEscape(filename)}" path="/" encoding="base64">${bytesToBase64(image.bytes)}</file>`;
    }
    output += `</questiontext>\n`;
    output += `    <single>${correctCount === 1 ? "true" : "false"}</single>\n`;
    output += `    <shuffleanswers>true</shuffleanswers>\n`;
    for (const [index, answer] of question.answers.entries()) {
      output += `    <answer fraction="${formatFraction(fractions[index])}"><text><![CDATA[${renderInlines(answer.content, images)}]]></text></answer>\n`;
    }
    output += `  </question>\n`;
  }
  return output + "</quiz>\n";
}

function renderInlines(inlines: Inline[], images: Map<string, EmbeddedImage>): string {
  return inlines.map(inline => {
    if (inline.type === "image") {
      const image = images.get(inline.name);
      const dimensions = image ? ` width="${image.widthPx}" height="${image.heightPx}"` : "";
      return `<img src="@@PLUGINFILE@@/${inline.name}" alt=""${dimensions}/>`;
    }
    let text = htmlEscape(inline.value);
    if (inline.italic) text = `<i>${text}</i>`;
    if (inline.bold) text = `<strong>${text}</strong>`;
    return text;
  }).join("");
}

function imageNames(paragraphs: Inline[][]): string[] {
  const names = new Set<string>();
  for (const paragraph of paragraphs) for (const inline of paragraph) if (inline.type === "image") names.add(inline.name);
  return [...names];
}

function plainText(paragraph: Paragraph): string {
  return paragraph.inlines.filter((inline): inline is TextInline => inline.type === "text").map(inline => inline.value).join("");
}

function isParagraphEmpty(paragraph: Paragraph): boolean {
  return paragraph.inlines.every(inline => inline.type === "text" ? inline.value.trim() === "" : false);
}

function emuToPx(emu: number): number {
  return Math.max(Math.round(emu / 9525), 1);
}

function parseNumber(value: string | undefined): number {
  const number = Number(value);
  return Number.isFinite(number) && number > 0 ? number : 0;
}

function boundedCrop(value: string | undefined): number {
  return Math.min(parseNumber(value), 100_000);
}

async function cropImage(bytes: Uint8Array, crop: Crop): Promise<Uint8Array> {
  if (crop.left === 0 && crop.top === 0 && crop.right === 0 && crop.bottom === 0) return bytes;
  const mime = imageMime(bytes);
  if (mime !== "image/jpeg" && mime !== "image/png") throw new Error(`Unsupported cropped image format ${mime}`);
  const bitmap = await createImageBitmap(new Blob([toArrayBuffer(bytes)], { type: mime }));
  try {
    const left = Math.min(Math.floor(bitmap.width * crop.left / 100_000), bitmap.width - 1);
    const top = Math.min(Math.floor(bitmap.height * crop.top / 100_000), bitmap.height - 1);
    const right = Math.max(left + 1, bitmap.width - Math.floor(bitmap.width * crop.right / 100_000));
    const bottom = Math.max(top + 1, bitmap.height - Math.floor(bitmap.height * crop.bottom / 100_000));
    const width = Math.min(right - left, bitmap.width - left);
    const height = Math.min(bottom - top, bitmap.height - top);
    const canvas = new OffscreenCanvas(width, height);
    const context = canvas.getContext("2d");
    if (!context) throw new Error("Could not create an image canvas");
    context.drawImage(bitmap, left, top, width, height, 0, 0, width, height);
    const blob = await canvas.convertToBlob({ type: mime, ...(mime === "image/jpeg" ? { quality: 0.75 } : {}) });
    return new Uint8Array(await blob.arrayBuffer());
  } finally {
    bitmap.close();
  }
}

function imageMime(bytes: Uint8Array): string {
  if (bytes[0] === 0x89 && bytes[1] === 0x50 && bytes[2] === 0x4e && bytes[3] === 0x47) return "image/png";
  if (bytes[0] === 0xff && bytes[1] === 0xd8) return "image/jpeg";
  if (bytes[0] === 0x47 && bytes[1] === 0x49 && bytes[2] === 0x46) return "image/gif";
  if (bytes[0] === 0x42 && bytes[1] === 0x4d) return "image/bmp";
  return "application/octet-stream";
}

function toArrayBuffer(bytes: Uint8Array): ArrayBuffer {
  return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer;
}

function xmlEscape(value: string): string {
  return value.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;").replace(/'/g, "&apos;");
}

function htmlEscape(value: string): string {
  return value.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

function formatFraction(value: number): string {
  return value.toFixed(10).replace(/0+$/, "").replace(/\.$/, "");
}
