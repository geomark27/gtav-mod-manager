import { formatBytes, formatDateTime } from "./format";

describe("formatBytes", () => {
  it.each<[number, string]>([
    [0, "0 B"],
    [-5, "0 B"],
    [Number.NaN, "0 B"],
    [1, "1 B"],
    [1023, "1023 B"],
    [1024, "1.0 KB"],
    [1536, "1.5 KB"],
    [10 * 1024, "10 KB"],
    [5 * 1024 * 1024, "5.0 MB"],
    [3.5 * 1024 ** 3, "3.5 GB"],
    [2 * 1024 ** 4, "2.0 TB"],
    [5000 * 1024 ** 4, "5000 TB"],
  ])("formats %d as %s", (input, expected) => {
    expect(formatBytes(input)).toBe(expected);
  });
});

describe("formatDateTime", () => {
  it("formats an RFC 3339 timestamp in the requested zone", () => {
    const text = formatDateTime("2026-01-05T15:04:00Z", "UTC");
    expect(text).toMatch(/Jan 5, 2026/);
    expect(text).toMatch(/3:04/);
  });

  it("returns the input when it is not a date", () => {
    expect(formatDateTime("not a date")).toBe("not a date");
  });
});
