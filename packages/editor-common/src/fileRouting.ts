export type SupportedFileType = "redoc" | "csv" | "xlsx" | "docx" | "pptx";

/** Resolve a local path to the file adapter that should handle it. */
export function supportedFileType(path: string): SupportedFileType | null {
  const name = path.trim().split(/[\\/]/).pop() || "";
  const extension = name.includes(".") ? name.slice(name.lastIndexOf(".") + 1).toLowerCase() : "";
  return ["redoc", "csv", "xlsx", "docx", "pptx"].includes(extension)
    ? (extension as SupportedFileType)
    : null;
}
