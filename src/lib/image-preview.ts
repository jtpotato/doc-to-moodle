/** Converts a data URL to a blob URL without assuming how its data is encoded. */
export async function createImagePreviewUrl(src: string): Promise<string> {
  const response = await fetch(src);
  if (!response.ok) throw new Error("Could not read embedded image data.");

  const fetchedImage = await response.blob();
  const mime = /^data:([^;,]+)/i.exec(src)?.[1];
  const image =
    fetchedImage.type || !mime
      ? fetchedImage
      : new Blob([await fetchedImage.arrayBuffer()], { type: mime });
  if (image.size === 0) throw new Error("Embedded image data is empty.");

  return URL.createObjectURL(image);
}
