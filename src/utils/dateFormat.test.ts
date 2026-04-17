import { describe, it, expect } from "vitest";
import { formatDateTime, formatDate, formatRelativeTime } from "./dateFormat";

describe("formatDateTime", () => {
  it("formats a valid Unix timestamp", () => {
    // 2024-01-15 12:00:00 UTC = 1705320000
    const result = formatDateTime("1705320000", "en-US");
    expect(result).toContain("2024");
    expect(result).toContain("January");
    expect(result).toContain("15");
  });

  it("returns original string for invalid timestamp", () => {
    expect(formatDateTime("not-a-number", "en")).toBe("not-a-number");
  });

  it("returns original string for NaN", () => {
    expect(formatDateTime("NaN", "en")).toBe("NaN");
  });

  it("handles zero timestamp (epoch)", () => {
    const result = formatDateTime("0", "en-US");
    // Epoch displays as Dec 31 1969 or Jan 1 1970 depending on local timezone
    expect(result).toMatch(/1969|1970/);
  });
});

describe("formatDate", () => {
  it("formats a valid Unix timestamp without time", () => {
    const result = formatDate("1705320000", "en-US");
    expect(result).toContain("2024");
    expect(result).toContain("January");
    // Should not contain time components
    expect(result).not.toMatch(/\d{1,2}:\d{2}/);
  });

  it("returns original string for invalid timestamp", () => {
    expect(formatDate("garbage", "en")).toBe("garbage");
  });
});

describe("formatRelativeTime", () => {
  it("returns original string for invalid timestamp", () => {
    expect(formatRelativeTime("invalid", "en")).toBe("invalid");
  });

  it("handles a very old timestamp (years ago)", () => {
    // Year 2000 timestamp
    const result = formatRelativeTime("946684800", "en");
    expect(result).toMatch(/year/i);
  });
});
