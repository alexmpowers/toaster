import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

// Configuration
const LOCALES_DIR = path.join(__dirname, "..", "src", "i18n", "locales");
const REFERENCE_LANG = "en";
const SRC_DIR = path.join(__dirname, "..", "src");
const KEY_USAGE_IGNORE_PATH = path.join(__dirname, "i18n-key-usage-ignore.txt");
const CHECK_KEY_USAGE =
  process.argv.includes("--check-key-usage") ||
  process.argv.includes("--strict-key-usage");
const STRICT_KEY_USAGE = process.argv.includes("--strict-key-usage");
const STRICT_UNUSED_KEYS = process.argv.includes("--strict-unused-keys");

type TranslationData = Record<string, unknown>;

interface ValidationResult {
  valid: boolean;
  missing: string[][];
  extra: string[][];
}

interface KeyUsageResult {
  keysReferencedInCode: number;
  templatePrefixesReferencedInCode: number;
  inferredKeyCandidatesInCode: number;
  ignoredDerivedPluralKeys: number;
  ignoredByPolicy: number;
  missingInReference: string[];
  missingTemplatePrefixes: string[];
  unreferencedInCode: string[];
}

const PLURAL_SUFFIXES = [
  "_zero",
  "_one",
  "_two",
  "_few",
  "_many",
  "_other",
  "_plural",
];

function loadKeyUsageIgnorePatterns(): string[] {
  if (!fs.existsSync(KEY_USAGE_IGNORE_PATH)) return [];
  const raw = fs.readFileSync(KEY_USAGE_IGNORE_PATH, "utf8");
  return raw
    .split(/\r?\n/)
    .map((line) => line.replace(/#.*$/, "").trim())
    .filter((line) => line.length > 0);
}

function matchesIgnorePattern(key: string, pattern: string): boolean {
  if (pattern.endsWith("*")) {
    const prefix = pattern.slice(0, -1);
    return key.startsWith(prefix);
  }
  return key === pattern;
}

function getLanguages(): string[] {
  const entries = fs.readdirSync(LOCALES_DIR, { withFileTypes: true });
  return entries
    .filter((entry) => entry.isDirectory() && entry.name !== REFERENCE_LANG)
    .map((entry) => entry.name)
    .sort();
}

const LANGUAGES = getLanguages();

// Colors for terminal output
const colors: Record<string, string> = {
  reset: "\x1b[0m",
  red: "\x1b[31m",
  green: "\x1b[32m",
  yellow: "\x1b[33m",
  blue: "\x1b[34m",
};

function colorize(text: string, color: string): string {
  return `${colors[color]}${text}${colors.reset}`;
}

function getAllKeyPaths(
  obj: TranslationData,
  prefix: string[] = [],
): string[][] {
  let paths: string[][] = [];
  for (const key in obj) {
    if (!Object.hasOwn(obj, key)) continue;

    const currentPath = prefix.concat([key]);
    const value = obj[key];

    if (typeof value === "object" && value !== null && !Array.isArray(value)) {
      paths = paths.concat(
        getAllKeyPaths(value as TranslationData, currentPath),
      );
    } else {
      paths.push(currentPath);
    }
  }
  return paths;
}

function hasKeyPath(obj: TranslationData, keyPath: string[]): boolean {
  let current: unknown = obj;
  for (const key of keyPath) {
    if (
      typeof current !== "object" ||
      current === null ||
      (current as Record<string, unknown>)[key] === undefined
    ) {
      return false;
    }
    current = (current as Record<string, unknown>)[key];
  }
  return true;
}

function loadTranslationFile(lang: string): TranslationData | null {
  const filePath = path.join(LOCALES_DIR, lang, "translation.json");

  try {
    const content = fs.readFileSync(filePath, "utf8");
    return JSON.parse(content) as TranslationData;
  } catch (error) {
    console.error(colorize(`✗ Error loading ${lang}/translation.json:`, "red"));
    console.error(`  ${(error as Error).message}`);
    return null;
  }
}

function walkSourceFiles(dir: string, out: string[]): void {
  let entries: fs.Dirent[];
  try {
    entries = fs.readdirSync(dir, { withFileTypes: true });
  } catch {
    return;
  }

  for (const entry of entries) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      if (
        entry.name === "node_modules" ||
        entry.name === "dist" ||
        entry.name === "target" ||
        entry.name === "i18n"
      ) {
        continue;
      }
      walkSourceFiles(full, out);
      continue;
    }

    if (!entry.isFile()) continue;
    if (!/\.(ts|tsx|js|jsx)$/.test(entry.name)) continue;
    out.push(full);
  }
}

function collectTranslationKeyUsage(): {
  usedLiteralKeys: Set<string>;
  usedTemplatePrefixes: Set<string>;
  inferredKeyCandidates: Set<string>;
} {
  const files: string[] = [];
  walkSourceFiles(SRC_DIR, files);

  const usedLiteralKeys = new Set<string>();
  const usedTemplatePrefixes = new Set<string>();
  const inferredKeyCandidates = new Set<string>();
  const literalPatterns = [
    /\bi18n\.t\s*\(\s*["'`]([a-zA-Z0-9_.-]+)["'`]/g,
    /\bt\s*\(\s*["'`]([a-zA-Z0-9_.-]+)["'`]/g,
  ];
  const templatePatterns = [
    /\bi18n\.t\s*\(\s*`([a-zA-Z0-9_.-]+)\$\{/g,
    /\bt\s*\(\s*`([a-zA-Z0-9_.-]+)\$\{/g,
  ];
  // Heuristic: dotted string literals often used in titleKey/labelKey maps.
  // We only apply these as extra coverage for unreferenced-key detection,
  // not as hard missing-key evidence.
  const inferredPatterns = [
    /["'`]([a-z][a-z0-9_-]*(?:\.[a-zA-Z0-9_-]+){1,})["'`]/g,
  ];

  for (const file of files) {
    let content = "";
    try {
      content = fs.readFileSync(file, "utf8");
    } catch {
      continue;
    }

    for (const pattern of literalPatterns) {
      for (const match of content.matchAll(pattern)) {
        const key = match[1]?.trim();
        if (key) usedLiteralKeys.add(key);
      }
    }

    for (const pattern of templatePatterns) {
      for (const match of content.matchAll(pattern)) {
        const prefix = match[1]?.trim();
        if (prefix) usedTemplatePrefixes.add(prefix);
      }
    }

    for (const pattern of inferredPatterns) {
      for (const match of content.matchAll(pattern)) {
        const candidate = match[1]?.trim();
        if (candidate) inferredKeyCandidates.add(candidate);
      }
    }
  }

  return { usedLiteralKeys, usedTemplatePrefixes, inferredKeyCandidates };
}

function evaluateKeyUsage(referenceKeyPaths: string[][]): KeyUsageResult {
  const ignorePatterns = loadKeyUsageIgnorePatterns();
  const referenceKeyList = referenceKeyPaths.map((k) => k.join("."));
  const referenceKeys = new Set(referenceKeyList);
  const { usedLiteralKeys, usedTemplatePrefixes, inferredKeyCandidates } =
    collectTranslationKeyUsage();

  const isCoveredByTemplatePrefix = (key: string): boolean => {
    for (const prefix of usedTemplatePrefixes) {
      if (key.startsWith(prefix)) return true;
    }
    return false;
  };

  const toPluralBaseKey = (key: string): string | null => {
    for (const suffix of PLURAL_SUFFIXES) {
      if (key.endsWith(suffix)) {
        return key.slice(0, key.length - suffix.length);
      }
    }
    return null;
  };

  const missingInReference = [...usedLiteralKeys]
    .filter((key) => !referenceKeys.has(key))
    .sort();

  const missingTemplatePrefixes = [...usedTemplatePrefixes]
    .filter((prefix) => !referenceKeyList.some((k) => k.startsWith(prefix)))
    .sort();

  let ignoredDerivedPluralKeys = 0;
  let ignoredByPolicy = 0;
  const unreferencedInCode = [...referenceKeys]
    .filter((key) => {
      if (usedLiteralKeys.has(key)) return false;
      if (inferredKeyCandidates.has(key)) return false;
      if (isCoveredByTemplatePrefix(key)) return false;

      const base = toPluralBaseKey(key);
      if (base) {
        const baseCovered =
          usedLiteralKeys.has(base) ||
          inferredKeyCandidates.has(base) ||
          isCoveredByTemplatePrefix(base);
        if (baseCovered) {
          ignoredDerivedPluralKeys++;
          return false;
        }
      }

      if (
        ignorePatterns.some((pattern) => matchesIgnorePattern(key, pattern))
      ) {
        ignoredByPolicy++;
        return false;
      }

      return true;
    })
    .sort();

  return {
    keysReferencedInCode: usedLiteralKeys.size,
    templatePrefixesReferencedInCode: usedTemplatePrefixes.size,
    inferredKeyCandidatesInCode: inferredKeyCandidates.size,
    ignoredDerivedPluralKeys,
    ignoredByPolicy,
    missingInReference,
    missingTemplatePrefixes,
    unreferencedInCode,
  };
}

function validateTranslations(): void {
  console.log(colorize("\n🌍 Translation Consistency Check\n", "blue"));

  // Load reference file
  console.log(`Loading reference language: ${REFERENCE_LANG}`);
  const referenceData = loadTranslationFile(REFERENCE_LANG);

  if (!referenceData) {
    console.error(
      colorize(`\n✗ Failed to load reference file (${REFERENCE_LANG})`, "red"),
    );
    process.exit(1);
  }

  // Get all key paths from reference
  const referenceKeyPaths = getAllKeyPaths(referenceData);
  console.log(`Reference has ${referenceKeyPaths.length} keys\n`);

  // Track validation results
  let hasErrors = false;
  const results: Record<string, ValidationResult> = {};

  // Validate each language
  for (const lang of LANGUAGES) {
    const langData = loadTranslationFile(lang);

    if (!langData) {
      hasErrors = true;
      results[lang] = { valid: false, missing: [], extra: [] };
      continue;
    }

    // Find missing keys
    const missing = referenceKeyPaths.filter(
      (keyPath) => !hasKeyPath(langData, keyPath),
    );

    // Find extra keys (keys in language but not in reference)
    const langKeyPaths = getAllKeyPaths(langData);
    const extra = langKeyPaths.filter(
      (keyPath) => !hasKeyPath(referenceData, keyPath),
    );

    results[lang] = {
      valid: missing.length === 0 && extra.length === 0,
      missing,
      extra,
    };

    if (missing.length > 0 || extra.length > 0) {
      hasErrors = true;
    }
  }

  // Print results
  console.log(colorize("Results:", "blue"));
  console.log("─".repeat(60));

  for (const lang of LANGUAGES) {
    const result = results[lang];

    if (result.valid) {
      console.log(
        colorize(`✓ ${lang.toUpperCase()}: All keys present`, "green"),
      );
    } else {
      console.log(colorize(`✗ ${lang.toUpperCase()}: Issues found`, "red"));

      if (result.missing.length > 0) {
        console.log(
          colorize(`  Missing ${result.missing.length} keys:`, "yellow"),
        );
        result.missing.slice(0, 10).forEach((keyPath) => {
          console.log(`    - ${keyPath.join(".")}`);
        });
        if (result.missing.length > 10) {
          console.log(
            colorize(
              `    ... and ${result.missing.length - 10} more`,
              "yellow",
            ),
          );
        }
      }

      if (result.extra.length > 0) {
        console.log(
          colorize(
            `  Extra ${result.extra.length} keys (not in reference):`,
            "yellow",
          ),
        );
        result.extra.slice(0, 10).forEach((keyPath) => {
          console.log(`    - ${keyPath.join(".")}`);
        });
        if (result.extra.length > 10) {
          console.log(
            colorize(`    ... and ${result.extra.length - 10} more`, "yellow"),
          );
        }
      }

      console.log("");
    }
  }

  console.log("─".repeat(60));

  // Summary
  const validCount = Object.values(results).filter((r) => r.valid).length;
  const totalCount = LANGUAGES.length;

  let keyUsageFailed = false;

  if (CHECK_KEY_USAGE) {
    const usage = evaluateKeyUsage(referenceKeyPaths);

    console.log(colorize("\nKey Usage Audit:", "blue"));
    console.log("─".repeat(60));
    console.log(
      `Literal translation keys referenced in code: ${usage.keysReferencedInCode}`,
    );
    console.log(
      `Template-key prefixes referenced in code: ${usage.templatePrefixesReferencedInCode}`,
    );
    console.log(
      `Inferred dotted key candidates in code: ${usage.inferredKeyCandidatesInCode}`,
    );
    console.log(
      `Ignored derived plural keys: ${usage.ignoredDerivedPluralKeys}`,
    );
    console.log(`Ignored by key-usage policy: ${usage.ignoredByPolicy}`);

    if (usage.missingInReference.length > 0) {
      const status = STRICT_KEY_USAGE ? "✗" : "⚠";
      const color = STRICT_KEY_USAGE ? "red" : "yellow";
      console.log(
        colorize(
          `${status} Referenced in code but missing in ${REFERENCE_LANG}: ${usage.missingInReference.length}`,
          color,
        ),
      );
      usage.missingInReference.slice(0, 20).forEach((key) => {
        console.log(`    - ${key}`);
      });
      if (usage.missingInReference.length > 20) {
        console.log(
          colorize(
            `    ... and ${usage.missingInReference.length - 20} more`,
            color,
          ),
        );
      }
    } else {
      console.log(
        colorize(
          `✓ No missing ${REFERENCE_LANG} keys referenced from code`,
          "green",
        ),
      );
    }

    if (usage.missingTemplatePrefixes.length > 0) {
      const status = STRICT_KEY_USAGE ? "✗" : "⚠";
      const color = STRICT_KEY_USAGE ? "red" : "yellow";
      console.log(
        colorize(
          `${status} Template key prefixes used in code but unmatched in ${REFERENCE_LANG}: ${usage.missingTemplatePrefixes.length}`,
          color,
        ),
      );
      usage.missingTemplatePrefixes.slice(0, 20).forEach((prefix) => {
        console.log(`    - ${prefix}*`);
      });
      if (usage.missingTemplatePrefixes.length > 20) {
        console.log(
          colorize(
            `    ... and ${usage.missingTemplatePrefixes.length - 20} more`,
            color,
          ),
        );
      }
    } else {
      console.log(
        colorize(
          `✓ No missing template-key prefixes in ${REFERENCE_LANG}`,
          "green",
        ),
      );
    }

    if (usage.unreferencedInCode.length > 0) {
      const status = STRICT_KEY_USAGE ? "✗" : "⚠";
      const color = STRICT_KEY_USAGE ? "red" : "yellow";
      console.log(
        colorize(
          `${status} Present in ${REFERENCE_LANG} but not found as literal t()/i18n.t() usage: ${usage.unreferencedInCode.length}`,
          color,
        ),
      );
      usage.unreferencedInCode.slice(0, 20).forEach((key) => {
        console.log(`    - ${key}`);
      });
      if (usage.unreferencedInCode.length > 20) {
        console.log(
          colorize(
            `    ... and ${usage.unreferencedInCode.length - 20} more`,
            color,
          ),
        );
      }
    } else {
      console.log(
        colorize(
          `✓ No unreferenced ${REFERENCE_LANG} keys detected from literal usage scan`,
          "green",
        ),
      );
    }

    const shouldFailOnMissing = STRICT_KEY_USAGE;
    const shouldFailOnUnused = STRICT_KEY_USAGE && STRICT_UNUSED_KEYS;
    if (
      (shouldFailOnMissing &&
        (usage.missingInReference.length > 0 ||
          usage.missingTemplatePrefixes.length > 0)) ||
      (shouldFailOnUnused && usage.unreferencedInCode.length > 0)
    ) {
      keyUsageFailed = true;
    }

    if (!STRICT_KEY_USAGE) {
      console.log(
        colorize(
          "(Report-only mode for key usage. Use --strict-key-usage to fail on usage drift.)",
          "yellow",
        ),
      );
    } else if (!STRICT_UNUSED_KEYS) {
      console.log(
        colorize(
          "(Strict mode currently fails on missing referenced keys only. Pass --strict-unused-keys to also fail on unreferenced keys.)",
          "yellow",
        ),
      );
    }
  }

  if (hasErrors || keyUsageFailed) {
    console.log(
      colorize(
        `\n✗ Validation failed: ${validCount}/${totalCount} languages passed`,
        "red",
      ),
    );
    if (keyUsageFailed) {
      console.log(colorize("✗ Key usage audit failed in strict mode", "red"));
    }
    process.exit(1);
  } else {
    console.log(
      colorize(
        `\n✓ All ${totalCount} languages have complete translations!`,
        "green",
      ),
    );
    process.exit(0);
  }
}

// Run validation
validateTranslations();
