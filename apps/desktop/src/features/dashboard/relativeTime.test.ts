import { formatRelative } from "./relativeTime";

const NOW = Date.parse("2026-03-01T12:00:00Z");

describe("formatRelative", () => {
  it("formats minutes, hours and days", () => {
    expect(formatRelative("2026-03-01T11:59:40Z", NOW)).toBe("just now");
    expect(formatRelative("2026-03-01T11:59:00Z", NOW)).toBe("1 minute ago");
    expect(formatRelative("2026-03-01T11:30:00Z", NOW)).toBe("30 minutes ago");
    expect(formatRelative("2026-03-01T09:00:00Z", NOW)).toBe("3 hours ago");
    expect(formatRelative("2026-02-27T12:00:00Z", NOW)).toBe("2 days ago");
  });

  it("falls back to the date for old or invalid values", () => {
    expect(formatRelative("2025-01-01T12:00:00Z", NOW)).toMatch(/2025/);
    expect(formatRelative("nope", NOW)).toBe("nope");
  });
});
