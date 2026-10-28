"use client";

import { useEffect, useState } from "react";
import type { Inline } from "@/lib/converter";
import { createImagePreviewUrl } from "@/lib/image-preview";

export function InlineContent({ items }: { items: Inline[] }) {
  return (
    <>
      {items.map((item, index) =>
        item.type === "image" ? (
          <EmbeddedImage key={index} src={item.src} />
        ) : (
          <span key={index}>
            {item.bold && item.italic ? (
              <strong><em>{item.value}</em></strong>
            ) : item.bold ? (
              <strong>{item.value}</strong>
            ) : item.italic ? (
              <em>{item.value}</em>
            ) : (
              item.value
            )}
          </span>
        ),
      )}
    </>
  );
}

function EmbeddedImage({ src }: { src?: string }) {
  const [previewSrc, setPreviewSrc] = useState<string>();

  useEffect(() => {
    if (!src) return;

    let active = true;
    let objectUrl: string | undefined;

    createImagePreviewUrl(src)
      .then((url) => {
        objectUrl = url;
        if (active) setPreviewSrc(url);
        else URL.revokeObjectURL(url);
      })
      .catch(() => {
        if (active) setPreviewSrc(src);
      });

    return () => {
      active = false;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [src]);

  if (!src) return null;

  return (
    <img
      className="my-3 block h-auto max-h-72 w-auto max-w-full rounded-md border object-contain"
      src={previewSrc || src}
      alt="Embedded document image"
    />
  );
}
