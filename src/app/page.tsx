"use client";

import { ChangeEvent, DragEvent, FormEvent, useRef, useState } from "react";
import {
  CheckCircle2,
  Download,
  FileText,
  LoaderCircle,
  LockKeyhole,
  Upload,
  X,
} from "lucide-react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Separator } from "@/components/ui/separator";
import { InlineContent } from "@/components/inline-content";
import { answerFractions, type ConversionResult } from "@/lib/converter";

type Result = ConversionResult;

export default function Home() {
  const [file, setFile] = useState<File | null>(null);
  const [result, setResult] = useState<Result | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [dragging, setDragging] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  function selectFile(next: File | undefined) {
    setError("");
    setResult(null);
    if (!next) return;
    if (!next.name.toLowerCase().endsWith(".docx")) {
      setFile(null);
      setError("Choose a Word document ending in .docx.");
      return;
    }
    if (next.size > 50 * 1024 * 1024) {
      setFile(null);
      setError("This document is larger than the 50 MB limit.");
      return;
    }
    setFile(next);
  }

  async function convert(event: FormEvent) {
    event.preventDefault();
    if (!file) return;
    setBusy(true);
    setError("");
    setResult(null);
    let worker: Worker | null = null;
    try {
      const bytes = await file.arrayBuffer();
      const conversionWorker = new Worker(
        new URL("../workers/converter.worker.ts", import.meta.url),
      );
      worker = conversionWorker;
      const converted = await new Promise<Result>((resolve, reject) => {
        conversionWorker.onmessage = (message: MessageEvent) =>
          message.data.ok
            ? resolve(message.data.result)
            : reject(new Error(message.data.error));
        conversionWorker.onerror = (message) =>
          reject(
            new Error(message.message || "The converter failed to start."),
          );
        conversionWorker.postMessage(bytes, [bytes]);
      });
      setResult(converted);
      window.setTimeout(
        () =>
          document
            .getElementById("preview")
            ?.scrollIntoView({ behavior: "smooth", block: "start" }),
        50,
      );
    } catch (reason) {
      setError(
        reason instanceof Error
          ? reason.message
          : "Something went wrong while converting the document.",
      );
    } finally {
      worker?.terminate();
      setBusy(false);
    }
  }

  function download() {
    if (!result) return;
    const url = URL.createObjectURL(
      new Blob([result.xml], { type: "application/xml;charset=utf-8" }),
    );
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = `${file?.name.replace(/\.docx$/i, "") || "questions"}.xml`;
    anchor.click();
    URL.revokeObjectURL(url);
  }

  function onDrop(event: DragEvent) {
    event.preventDefault();
    setDragging(false);
    selectFile(event.dataTransfer.files[0]);
  }
  function onChange(event: ChangeEvent<HTMLInputElement>) {
    selectFile(event.target.files?.[0]);
  }

  return (
    <main className="mx-auto min-h-screen w-full max-w-4xl px-4 pb-12 pt-6 sm:px-6 md:pt-10">
      <section className="mx-auto max-w-3xl space-y-6">
        <Card>
          <CardHeader>
            <CardTitle className="text-base">Upload document</CardTitle>
          </CardHeader>
          <CardContent>
            <form onSubmit={convert} className="space-y-4">
              <div
                className={`rounded-lg border border-dashed p-6 transition-colors ${dragging ? "border-ring bg-muted/50" : ""}`}
                onDragOver={(e) => {
                  e.preventDefault();
                  setDragging(true);
                }}
                onDragLeave={() => setDragging(false)}
                onDrop={onDrop}
              >
                <Input
                  ref={inputRef}
                  type="file"
                  accept=".docx,application/vnd.openxmlformats-officedocument.wordprocessingml.document"
                  onChange={onChange}
                  className="hidden"
                />
                {file ? (
                  <div className="flex items-center gap-3">
                    <div className="rounded-md bg-muted p-2">
                      <FileText className="size-5 text-muted-foreground" />
                    </div>
                    <div className="min-w-0 flex-1">
                      <p className="truncate text-sm font-medium">
                        {file.name}
                      </p>
                      <p className="text-xs text-muted-foreground">
                        {(file.size / 1024 / 1024).toFixed(1)} MB
                      </p>
                    </div>
                    <Button
                      type="button"
                      size="icon"
                      variant="ghost"
                      aria-label="Remove file"
                      onClick={() => {
                        setFile(null);
                        setResult(null);
                      }}
                    >
                      <X />
                    </Button>
                  </div>
                ) : (
                  <div className="flex flex-col items-center gap-3 text-center">
                    <div className="rounded-full border bg-background p-3">
                      <Upload className="size-5 text-muted-foreground" />
                    </div>
                    <div className="space-y-1">
                      <p className="text-sm font-medium">
                        Drop your document here
                      </p>
                      <p className="text-sm text-muted-foreground">
                        or select a file from your device
                      </p>
                    </div>
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      onClick={() => inputRef.current?.click()}
                    >
                      Browse files
                    </Button>
                  </div>
                )}
              </div>

              <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                <p className="text-xs text-muted-foreground">
                  DOCX only <span aria-hidden="true">·</span> Maximum 50 MB
                </p>
                <Button type="submit" disabled={!file || busy}>
                  {busy ? (
                    <>
                      <LoaderCircle className="animate-spin" /> Converting…
                    </>
                  ) : (
                    <>
                      Convert document <span aria-hidden="true">→</span>
                    </>
                  )}
                </Button>
              </div>
            </form>
          </CardContent>
        </Card>

        <div className="flex items-center gap-2 text-xs text-muted-foreground">
          <LockKeyhole className="size-3.5" /> Your document stays on this
          device.
        </div>
        {error && (
          <Alert variant="destructive">
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        )}
      </section>

      {result ? (
        <section className="mx-auto mt-14 max-w-3xl space-y-5" id="preview">
          <div className="flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between">
            <div className="space-y-1">
              <h2 className="text-2xl font-semibold tracking-tight">
                Review {result.questions.length} questions
              </h2>
            </div>
            <Button onClick={download}>
              <Download /> Download Moodle XML
            </Button>
          </div>
          <div className="space-y-4">
            {result.questions.map((question, index) => {
              const fractions = answerFractions(question.answers);
              return (
              <Card key={`${question.name}-${index}`}>
                <CardHeader className="flex flex-row items-center gap-3 space-y-0 border-b">
                  {/* <Badge variant="outline" className="tabular-nums">
                    {index + 1}
                  </Badge> */}
                  <CardTitle className="min-w-0 flex-1 truncate text-sm">
                    {question.name || `Question ${index + 1}`}
                  </CardTitle>
                  <Badge variant="secondary" className="font-normal">
                    Multiple choice
                  </Badge>
                </CardHeader>
                <CardContent className="space-y-4">
                  <div>
                    <div className="space-y-2 text-sm leading-6">
                      {question.prompt.map((paragraph, i) => (
                        <p key={i}>
                          <InlineContent items={paragraph} />
                        </p>
                      ))}
                    </div>
                  </div>
                  <Separator />
                  <div>
                    <div className="mb-2 flex items-center justify-between">
                      <p className="text-xs text-muted-foreground">
                        {question.answers.length} options
                      </p>
                    </div>
                    <ul className="space-y-2">
                      {question.answers.map((answer, i) => (
                        <li
                          key={i}
                          className={`flex items-start gap-3 rounded-md border px-3 py-2 text-sm ${answer.correct ? "border-emerald-200 bg-emerald-50/70 dark:border-emerald-900 dark:bg-emerald-950/30" : ""}`}
                        >
                          {answer.correct ? (
                            <CheckCircle2 className="mt-0.5 size-4 shrink-0 text-emerald-700 dark:text-emerald-400" />
                          ) : (
                            <span className="mt-0.5 size-4 shrink-0 rounded-full border" />
                          )}
                          <span className="min-w-0 flex-1 leading-5">
                            <InlineContent items={answer.content} />
                          </span>
                          {answer.correct && (
                            <Badge
                              variant="outline"
                              className="border-emerald-300 text-emerald-800 dark:border-emerald-800 dark:text-emerald-300"
                            >
                              Correct · {Number.isInteger(fractions[i]) ? fractions[i] : fractions[i].toFixed(2)}%
                            </Badge>
                          )}
                        </li>
                      ))}
                    </ul>
                  </div>
                </CardContent>
              </Card>
              );
            })}
          </div>
          <div className="flex flex-col gap-3 border-t pt-5 sm:flex-row sm:items-center sm:justify-between">
            <Button onClick={download}>
              <Download /> Download Moodle XML
            </Button>
          </div>
        </section>
      ) : (
        <>
          <section className="mx-auto mt-14 max-w-3xl space-y-5" id="preview">
            <h2 className="text-xl font-semibold">
              Convert Word Documents to Moodle XML
            </h2>
            <p>
              This tool converts Word documents to Moodle XML format. It
              currently only supports multiple-choice questions. The parser
              lends a lot of syntax from Moodle's GIFT format.
            </p>
            <p>
              For instance, use ::QUESTIONTITLE:: to denote a question (and its
              title). ~%0% to indicate a 0-mark correct answer, and ~%50% for a
              50 percent correct answer.
            </p>
            <p>Use [Answers] to indicate a section with answers.</p>
            <p>Denote correct answers with (*).</p>
          </section>
        </>
      )}
    </main>
  );
}
