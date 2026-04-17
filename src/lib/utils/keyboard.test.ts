import { describe, it, expect } from "vitest";
import { getKeyName, formatKeyCombination, normalizeKey, type OSType } from "./keyboard";

// Helper to create a minimal KeyboardEvent-like object
function makeKeyEvent(overrides: Partial<KeyboardEvent>): KeyboardEvent {
  return {
    code: "",
    key: "",
    keyCode: 0,
    which: 0,
    ...overrides,
  } as KeyboardEvent;
}

describe("getKeyName", () => {
  describe("code-based key resolution", () => {
    it("maps letter keys (KeyA → a)", () => {
      expect(getKeyName(makeKeyEvent({ code: "KeyA" }))).toBe("a");
      expect(getKeyName(makeKeyEvent({ code: "KeyZ" }))).toBe("z");
    });

    it("maps digit keys (Digit0 → 0)", () => {
      expect(getKeyName(makeKeyEvent({ code: "Digit5" }))).toBe("5");
    });

    it("maps function keys (F1 → f1)", () => {
      expect(getKeyName(makeKeyEvent({ code: "F1" }))).toBe("f1");
      expect(getKeyName(makeKeyEvent({ code: "F12" }))).toBe("f12");
    });

    it("maps numpad keys", () => {
      expect(getKeyName(makeKeyEvent({ code: "Numpad7" }))).toBe("numpad 7");
      expect(getKeyName(makeKeyEvent({ code: "NumpadMultiply" }))).toBe("numpad *");
      expect(getKeyName(makeKeyEvent({ code: "NumpadAdd" }))).toBe("numpad +");
    });

    it("maps special keys", () => {
      expect(getKeyName(makeKeyEvent({ code: "Space" }))).toBe("space");
      expect(getKeyName(makeKeyEvent({ code: "Enter" }))).toBe("enter");
      expect(getKeyName(makeKeyEvent({ code: "Escape" }))).toBe("esc");
      expect(getKeyName(makeKeyEvent({ code: "Backspace" }))).toBe("backspace");
      expect(getKeyName(makeKeyEvent({ code: "Tab" }))).toBe("tab");
      expect(getKeyName(makeKeyEvent({ code: "Delete" }))).toBe("delete");
    });

    it("maps arrow keys", () => {
      expect(getKeyName(makeKeyEvent({ code: "ArrowUp" }))).toBe("up");
      expect(getKeyName(makeKeyEvent({ code: "ArrowDown" }))).toBe("down");
      expect(getKeyName(makeKeyEvent({ code: "ArrowLeft" }))).toBe("left");
      expect(getKeyName(makeKeyEvent({ code: "ArrowRight" }))).toBe("right");
    });

    it("maps punctuation keys", () => {
      expect(getKeyName(makeKeyEvent({ code: "Semicolon" }))).toBe(";");
      expect(getKeyName(makeKeyEvent({ code: "Comma" }))).toBe(",");
      expect(getKeyName(makeKeyEvent({ code: "Period" }))).toBe(".");
      expect(getKeyName(makeKeyEvent({ code: "Slash" }))).toBe("/");
    });
  });

  describe("OS-specific modifier names", () => {
    it("maps Alt → option on macOS", () => {
      expect(getKeyName(makeKeyEvent({ code: "AltLeft" }), "macos")).toBe("option");
    });

    it("maps Alt → alt on Windows", () => {
      expect(getKeyName(makeKeyEvent({ code: "AltLeft" }), "windows")).toBe("alt");
    });

    it("maps Meta → command on macOS", () => {
      expect(getKeyName(makeKeyEvent({ code: "MetaLeft" }), "macos")).toBe("command");
    });

    it("maps Meta → super on Windows/Linux", () => {
      expect(getKeyName(makeKeyEvent({ code: "MetaLeft" }), "windows")).toBe("super");
      expect(getKeyName(makeKeyEvent({ code: "MetaLeft" }), "linux")).toBe("super");
    });

    it("maps Shift consistently across platforms", () => {
      for (const os of ["macos", "windows", "linux"] as OSType[]) {
        expect(getKeyName(makeKeyEvent({ code: "ShiftLeft" }), os)).toBe("shift");
      }
    });
  });

  describe("fallback to e.key", () => {
    it("uses key property when code is empty", () => {
      expect(getKeyName(makeKeyEvent({ code: "", key: "a" }))).toBe("a");
    });

    it("maps special key names via key fallback", () => {
      expect(getKeyName(makeKeyEvent({ code: "", key: " " }))).toBe("space");
      expect(getKeyName(makeKeyEvent({ code: "", key: "Escape" }))).toBe("esc");
    });

    it("maps Meta key with OS context via key fallback", () => {
      expect(getKeyName(makeKeyEvent({ code: "", key: "Meta" }), "macos")).toBe("command");
      expect(getKeyName(makeKeyEvent({ code: "", key: "Meta" }), "windows")).toBe("win");
    });
  });

  describe("edge cases", () => {
    it("returns unknown-N when no code or key available", () => {
      expect(getKeyName(makeKeyEvent({ code: "", key: "", keyCode: 42 }))).toBe("unknown-42");
    });
  });
});

describe("formatKeyCombination", () => {
  it("formats a simple key combination", () => {
    expect(formatKeyCombination("shift+a", "windows")).toBe("Shift + A");
  });

  it("formats left/right modifier variants", () => {
    expect(formatKeyCombination("option_left+shift+space", "macos")).toBe(
      "Left Option + Shift + Space",
    );
  });

  it("returns empty string for empty input", () => {
    expect(formatKeyCombination("", "windows")).toBe("");
  });

  it("formats function keys correctly", () => {
    expect(formatKeyCombination("ctrl+f1", "windows")).toBe("Ctrl + F1");
  });
});

describe("normalizeKey", () => {
  it("strips left/right prefix from modifiers", () => {
    expect(normalizeKey("left shift")).toBe("shift");
    expect(normalizeKey("right ctrl")).toBe("ctrl");
  });

  it("returns non-modifier keys unchanged", () => {
    expect(normalizeKey("a")).toBe("a");
    expect(normalizeKey("space")).toBe("space");
    expect(normalizeKey("f1")).toBe("f1");
  });
});
