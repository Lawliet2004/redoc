import { describe, expect, it } from "vitest";
import { supportedFileType } from "./fileRouting";

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
