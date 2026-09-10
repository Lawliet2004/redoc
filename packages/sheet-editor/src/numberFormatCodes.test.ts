import { describe, expect, it } from "vitest";
import { formatWithCode, serialToDate } from "./numberFormatCodes";

describe("formatWithCode", () => {
  it("renders thousands separators", () => {
    expect(formatWithCode(1234567, "#,##0")).toBe("1,234,567");
    expect(formatWithCode(1234.5, "#,##0.00")).toBe("1,234.50");
  });

  it("renders configurable currency symbols", () => {
    expect(formatWithCode(1234.5, "$#,##0.00")).toBe("$1,234.50");
    expect(formatWithCode(1234, "€#,##0")).toBe("€1,234");
    expect(formatWithCode(-1234.5, "$#,##0.00")).toBe("-$1,234.50");
  });

  it("renders percent variants", () => {
    expect(formatWithCode(0.156, "0.0%")).toBe("15.6%");
    expect(formatWithCode(0.5, "0%")).toBe("50%");
  });

  it("renders 0-30 decimal places", () => {
    expect(formatWithCode(1, "0.000")).toBe("1.000");
    expect(formatWithCode(2, "0." + "0".repeat(30))).toBe("2." + "0".repeat(30));
  });

  it("renders date patterns", () => {
    // 45000 = 2023-03-15 in Excel serial days.
    expect(formatWithCode(45000, "yyyy-mm-dd")).toBe("2023-03-15");
    expect(formatWithCode(45000, "dd/mm/yyyy")).toBe("15/03/2023");
    expect(formatWithCode(45000, "mm/dd/yyyy")).toBe("03/15/2023");
  });

  it("renders time-of-day from the serial fraction", () => {
    expect(formatWithCode(45000.5, "yyyy-mm-dd h:mm")).toBe("2023-03-15 12:00");
    expect(formatWithCode(0.25, "h:mm")).toBe("6:00");
  });

  it("pads small integers to the pattern's zero count", () => {
    expect(formatWithCode(7, "000")).toBe("007");
  });

  it("keeps negative sections when provided", () => {
    expect(formatWithCode(-5, "0;(0)")).toBe("(5)");
  });

  it("round-trips the serial date helper", () => {
    expect(serialToDate(45000).toISOString()).toBe("2023-03-15T00:00:00.000Z");
  });

  it("renders scientific notation", () => {
    expect(formatWithCode(123456, "0.00E+00")).toBe("1.23E+05");
    expect(formatWithCode(0.000123, "0.00E-00")).toBe("1.23E-04");
    expect(formatWithCode(-123456, "0.00E+00")).toBe("-1.23E+05");
    expect(formatWithCode(0, "0.00E+00")).toBe("0.00E+00");
    expect(formatWithCode(5, "0.0E+0")).toBe("5.0E+0");
    expect(formatWithCode(9999999, "0.000E+00")).toBe("1.000E+07");
  });

  it("renders scientific notation with currency prefix", () => {
    expect(formatWithCode(1500, "$0.0E+0")).toBe("$1.5E+3");
  });
});
