/**
 * Unit tests for src/lib/csv.ts (operator-toolkit F5, §5.6 frozen artifact).
 * Pure, DOM-free — Node-context Playwright tests, same pattern as
 * no-hex.spec.ts / contract.spec.ts.
 */
import { expect, test } from "@playwright/test";
import { CSV_HEADER, escapeCell, toCsv } from "../src/lib/csv";

test.describe("escapeCell", () => {
  test("a value containing a comma is quoted", () => {
    expect(escapeCell("a,b")).toBe('"a,b"');
  });

  test('a JSON body containing " is quoted with doubled internal quotes', () => {
    const body = '{"a":"b"}';
    expect(escapeCell(body)).toBe('"{""a"":""b""}"');
  });

  test("a body containing a literal newline is quoted", () => {
    expect(escapeCell("line one\nline two")).toBe('"line one\nline two"');
  });

  test("a non-ASCII body round-trips byte-exact", () => {
    const s = "héllo…🎉";
    expect(escapeCell(s)).toBe(s);
  });

  test("architecture D12: '=cmd|' /c calc'!A1' guards but does NOT quote (no comma/quote/CR/LF)", () => {
    const formula = "=cmd|' /c calc'!A1";
    expect(escapeCell(formula)).toBe("'=cmd|' /c calc'!A1");
  });

  test("guard fires on =, +, -, @, TAB and CR as the first character", () => {
    expect(escapeCell("=1+1")).toBe("'=1+1");
    expect(escapeCell("+1")).toBe("'+1");
    expect(escapeCell("-1")).toBe("'-1");
    expect(escapeCell("@SUM(A1)")).toBe("'@SUM(A1)");
    expect(escapeCell("\tindented")).toBe("'\tindented");
  });

  test("a bare integer string is untouched (never guarded, never quoted)", () => {
    expect(escapeCell("201")).toBe("201");
    expect(escapeCell("0")).toBe("0");
  });

  test("guard runs before quoting: a guarded value that also needs quoting gets both", () => {
    expect(escapeCell("=a,b")).toBe('"\'=a,b"');
  });
});

test.describe("toCsv", () => {
  test("has a trailing CRLF after the final record and no BOM", () => {
    const csv = toCsv(CSV_HEADER, [["1", "GET", "/x", "200", "rule", "3", "{}", "", "{}", ""]]);
    expect(csv.endsWith("\r\n")).toBe(true);
    expect(csv.charCodeAt(0)).not.toBe(0xfeff);
    expect(csv.includes("﻿")).toBe(false);
  });

  test("uses CRLF between records, not bare LF", () => {
    const csv = toCsv(["a", "b"], [
      ["1", "2"],
      ["3", "4"],
    ]);
    expect(csv).toBe("a,b\r\n1,2\r\n3,4\r\n");
  });

  test("zero data rows still produces a header + trailing CRLF", () => {
    expect(toCsv(["a", "b"], [])).toBe("a,b\r\n");
  });
});
