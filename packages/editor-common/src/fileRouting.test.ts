import { describe, expect, it } from "vitest";
import { supportedFileType, verifyMagicBytes } from "./fileRouting";

const ZIP_BYTES = new Uint8Array([0x50, 0x4b, 0x03, 0x04, 0x14, 0x00]);
const TEXT_BYTES = new Uint8Array([0x68, 0x65, 0x6c, 0x6c, 0x6f]);
const UTF8_BOM_TEXT = new Uint8Array([0xef, 0xbb, 0xbf, 0x68, 0x69]);
const NULL_BYTES = new Uint8Array([0x68, 0x69, 0x00, 0x21]);
const NOT_ZIP = new Uint8Array([0x25, 0x50, 0x44, 0x46, 0x2d]);

describe("supportedFileType", () => {
  it("routes supported Office and Redoc extensions case-insensitively", () => {
    expect(supportedFileType("C:\\Docs\\Report.DOCX")).toBe("docx");
    expect(supportedFileType("/tmp/Budget.XLSX")).toBe("xlsx");
    expect(supportedFileType("slides.PpTx")).toBe("pptx");
    expect(supportedFileType("notes.redoc")).toBe("redoc");
  });

  it("rejects unsupported or extensionless paths", () => {
    expect(supportedFileType("archive.xlsm")).toBeNull();
    expect(supportedFileType("README")).toBeNull();
    expect(supportedFileType("C:\\Docs\\folder.")).toBeNull();
  });
});

describe("verifyMagicBytes", () => {
  it("accepts ZIP containers for redoc/docx/xlsx/pptx", () => {
    expect(verifyMagicBytes("redoc", ZIP_BYTES)).toBeNull();
    expect(verifyMagicBytes("docx", ZIP_BYTES)).toBeNull();
    expect(verifyMagicBytes("xlsx", ZIP_BYTES)).toBeNull();
    expect(verifyMagicBytes("pptx", ZIP_BYTES)).toBeNull();
  });

  it("rejects non-ZIP content routed as a ZIP-based format", () => {
    const mismatch = verifyMagicBytes("docx", NOT_ZIP);
    expect(mismatch).not.toBeNull();
    expect(mismatch?.reason).toContain("ZIP");
  });

  it("accepts plain and UTF-8 BOM text as CSV", () => {
    expect(verifyMagicBytes("csv", TEXT_BYTES)).toBeNull();
    expect(verifyMagicBytes("csv", UTF8_BOM_TEXT)).toBeNull();
  });

  it("rejects ZIP or NUL-containing content routed as CSV", () => {
    expect(verifyMagicBytes("csv", ZIP_BYTES)?.reason).toContain("ZIP");
    expect(verifyMagicBytes("csv", NULL_BYTES)?.reason).toContain("NUL");
  });
});
