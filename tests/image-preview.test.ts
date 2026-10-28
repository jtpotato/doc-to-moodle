import { afterEach, describe, expect, it } from "bun:test";
import { zipSync } from "fflate";
import { convertDocx } from "@/lib/converter";
import { bytesToBase64 } from "@/lib/image-data";
import { createImagePreviewUrl } from "@/lib/image-preview";

const objectUrls: string[] = [];

afterEach(() => {
  for (const url of objectUrls.splice(0)) URL.revokeObjectURL(url);
});

describe("embedded image preview data", () => {
  it("preserves an embedded DOCX image through conversion and Blob preview", async () => {
    const png = Uint8Array.from([
      0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x00,
    ]);
    const docx = zipSync({
      "word/document.xml": new TextEncoder().encode(documentXml),
      "word/_rels/document.xml.rels": new TextEncoder().encode(relationshipsXml),
      "word/media/image1.png": png,
    });
    const input = docx.buffer.slice(
      docx.byteOffset,
      docx.byteOffset + docx.byteLength,
    ) as ArrayBuffer;
    const converted = await convertDocx(input);
    const image = converted.questions[0].prompt.flat().find((item) => item.type === "image");
    if (!image || image.type !== "image" || !image.src) {
      throw new Error("Converted prompt did not contain an embedded image.");
    }

    expect(image.src).toStartWith("data:image/png;base64,");
    expect(image.src.slice(image.src.indexOf(",") + 1)).toBe(bytesToBase64(png));

    const previewUrl = await createImagePreviewUrl(image.src);
    objectUrls.push(previewUrl);
    const response = await fetch(previewUrl);
    const blobBytes = new Uint8Array(await response.arrayBuffer());

    expect(response.status).toBe(200);
    expect([...blobBytes]).toEqual([...png]);
  });
});

const documentXml = `<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
  xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
  xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"
  xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
  <w:body>
    <w:p><w:r><w:t>::image-test::</w:t></w:r></w:p>
    <w:p><w:r><w:drawing><wp:inline><wp:extent cx="9525" cy="9525"/>
      <a:graphic><a:graphicData><a:blip r:embed="rId1"/></a:graphicData></a:graphic>
    </wp:inline></w:drawing></w:r></w:p>
    <w:p><w:r><w:t>[Answers]</w:t></w:r></w:p>
    <w:p><w:r><w:t>(* yes)</w:t></w:r></w:p>
  </w:body>
</w:document>`;

const relationshipsXml = `<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image1.png"/>
</Relationships>`;
