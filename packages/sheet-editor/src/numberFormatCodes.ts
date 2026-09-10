const pad2 = (n: number) => String(n).padStart(2, "0");

export function serialToDate(serial: number): Date {
  const epoch = new Date(Date.UTC(1899, 11, 30));
  epoch.setUTCDate(epoch.getUTCDate() + Math.floor(serial));
  return epoch;
}

/**
 * Renders an Excel-style format code against a numeric cell value.
 * Supports thousands separators (#,##0), currency symbols, percent,
 * decimal digits, negative sections, date/time tokens (yyyy, mm, dd, h),
 * and scientific notation (0.00E+00, 0.00E-00).
 */
export function formatWithCode(number: number, code: string): string {
  const [positive, negative] = code.split(";");
  const usedNegative = number < 0 && negative !== undefined;
  const section = (usedNegative ? negative! : positive).trim();
  const withoutLiterals = section.replace(/"[^"]*"/g, "");
  // Scientific notation: pattern contains 'E'/'e' followed by +/- and digits
  const hasScientific = /[eE][+\-]/.test(withoutLiterals) && /[#0]/.test(withoutLiterals);
  if (hasScientific) {
    return formatScientific(number, section);
  }
  const hasDateToken = /[ymdhs]/i.test(withoutLiterals) && !/[#0]/.test(withoutLiterals);
  if (hasDateToken) {
    const date = serialToDate(number);
    const isTimeOnly = !/[yd]/i.test(withoutLiterals) && /h/i.test(withoutLiterals);
    const timeOfDay = Math.abs(number % 1);
    if (isTimeOnly || timeOfDay > 0) {
      const totalMinutes = Math.round((isTimeOnly ? number : timeOfDay) * 24 * 60);
      const hours = Math.floor(totalMinutes / 60);
      const minutes = totalMinutes % 60;
      return section
        .replace(/yyyy/gi, String(date.getUTCFullYear()))
        .replace(/dd/gi, pad2(date.getUTCDate()))
        .replace(/hh/gi, pad2(hours))
        .replace(/h(?![a-z])/gi, String(hours))
        .replace(/:mm\b/g, `:${pad2(minutes)}`)
        .replace(/mm/gi, pad2(date.getUTCMonth() + 1));
    }
    return section
      .replace(/yyyy/gi, String(date.getUTCFullYear()))
      .replace(/dd/gi, pad2(date.getUTCDate()))
      .replace(/mm/gi, pad2(date.getUTCMonth() + 1));
  }
  let pattern = section;
  let literalPrefix = "";
  let literalSuffix = "";
  const prefixMatch = pattern.match(/^("[^"]*"|[^#0.,%]*)(?=[#0])/);
  if (prefixMatch && prefixMatch[1].length > 0) {
    literalPrefix = prefixMatch[1].replace(/^"|"$/g, "");
    pattern = pattern.slice(prefixMatch[1].length);
  }
  const isPercent = pattern.includes("%");
  pattern = pattern.replace(/%/g, "");
  const suffixMatch = pattern.match(/("[^"]*"|[^#0.,]*)$/);
  if (suffixMatch && suffixMatch[1].length > 0 && !/[#0]/.test(suffixMatch[1])) {
    literalSuffix = suffixMatch[1].replace(/^"|"$/g, "");
    pattern = pattern.slice(0, pattern.length - suffixMatch[1].length);
  }
  const value = Math.abs(number) * (isPercent ? 100 : 1);
  const dotIndex = pattern.indexOf(".");
  let decimalDigits = 0;
  if (dotIndex >= 0) {
    decimalDigits = pattern.length - dotIndex - 1;
    pattern = pattern.slice(0, dotIndex);
  }
  const useGrouping = pattern.includes(",");
  const minIntDigits = pattern.replace(/,/g, "").replace(/#/g, "").length;
  const fixed = value.toFixed(decimalDigits);
  let [intPart, decPart] = fixed.split(".");
  if (minIntDigits > 0) intPart = intPart.padStart(minIntDigits, "0");
  if (useGrouping) intPart = intPart.replace(/\B(?=(\d{3})+(?!\d))/g, ",");
  const rendered = decPart ? `${intPart}.${decPart}` : intPart;
  const percentSuffix = isPercent ? "%" : "";
  // A dedicated negative section carries its own sign/literals.
  const sign = number < 0 && !usedNegative ? "-" : "";
  return `${sign}${literalPrefix}${rendered}${literalSuffix}${percentSuffix}`;
}

/**
 * Formats a number in scientific notation (e.g., "1.23E+05").
 * Parses the pattern to determine decimal places and sign display.
 */
function formatScientific(number: number, section: string): string {
  // Extract literal prefix/suffix (currency symbols, etc.)
  const prefixMatch = section.match(/^("[^"]*"|[^#0eE]*)(?=[#0])/);
  let literalPrefix = "";
  let pattern = section;
  if (prefixMatch && prefixMatch[1].length > 0) {
    literalPrefix = prefixMatch[1].replace(/^"|"$/g, "");
    pattern = pattern.slice(prefixMatch[1].length);
  }
  const suffixMatch = pattern.match(/("[^"]*"|[^#0eE]*)$/);
  let literalSuffix = "";
  if (suffixMatch && suffixMatch[1].length > 0 && !/[#0]/.test(suffixMatch[1])) {
    literalSuffix = suffixMatch[1].replace(/^"|"$/g, "");
    pattern = pattern.slice(0, pattern.length - suffixMatch[1].length);
  }

  // Find the 'E' position and count decimal places before it
  const eIndex = pattern.search(/[eE]/);
  const decimalPart = pattern.slice(0, eIndex);
  const dotIndex = decimalPart.indexOf(".");
  const decimalPlaces = dotIndex >= 0 ? decimalPart.length - dotIndex - 1 : 0;

  // Determine exponent format: sign display and digit count from the pattern
  const ePart = pattern.slice(eIndex);
  const alwaysShowSign = /[eE]\+/.test(ePart) || /[eE]\-/.test(ePart);
  // Count zeros after the sign to determine exponent digit count
  const expDigitMatch = ePart.match(/[eE][+\-]?(0+)/);
  const expDigits = expDigitMatch ? expDigitMatch[1].length : 2;

  // Format the number in scientific notation
  if (number === 0) {
    const mantissa = decimalPlaces > 0 ? `0.${"0".repeat(decimalPlaces)}` : "0";
    const expSign = alwaysShowSign ? "+" : "";
    const expStr = "0".repeat(expDigits);
    return `${literalPrefix}${mantissa}E${expSign}${expStr}${literalSuffix}`;
  }

  const absNum = Math.abs(number);
  let exponent = Math.floor(Math.log10(absNum));
  // Adjust for cases like 0.01 where log10 gives -2 but we want -2
  let mantissa = absNum / Math.pow(10, exponent);
  // Handle floating point edge cases (e.g., 9.999999999999998)
  if (mantissa >= 10) {
    mantissa /= 10;
    exponent += 1;
  } else if (mantissa < 1) {
    mantissa *= 10;
    exponent -= 1;
  }

  // Round to specified decimal places, then re-check for carry (e.g., 9.999 → 10.000)
  let mantissaStr = mantissa.toFixed(decimalPlaces);
  if (mantissaStr.startsWith("10")) {
    mantissaStr = (mantissa / 10).toFixed(decimalPlaces);
    exponent += 1;
  }

  const sign = number < 0 ? "-" : "";
  const expSign = exponent >= 0 ? (alwaysShowSign ? "+" : "") : "-";
  const expStr = String(Math.abs(exponent)).padStart(expDigits, "0");

  return `${sign}${literalPrefix}${mantissaStr}E${expSign}${expStr}${literalSuffix}`;
}
