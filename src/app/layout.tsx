import type { Metadata } from "next";
import "./styles.css";

export const metadata: Metadata = {
  title: "DOCX to Moodle XML",
  description: "Convert a Word question document into Moodle XML, privately in your browser.",
};

export default function RootLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  return <html lang="en"><body>{children}</body></html>;
}
