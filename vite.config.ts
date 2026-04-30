import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { resolve } from "path";
import { readFileSync } from "fs";

const host = process.env.TAURI_DEV_HOST;

const pkg = JSON.parse(
  readFileSync(resolve(__dirname, "package.json"), "utf-8"),
) as { version?: string };

// https://vitejs.dev/config/
export default defineConfig(async () => ({
  plugins: [react(), tailwindcss()],

  // Path aliases
  resolve: {
    alias: {
      "@": resolve(__dirname, "./src"),
      "@/bindings": resolve(__dirname, "./src/bindings.ts"),
    },
  },

  // Inject the package version into the bundle as `import.meta.env.PACKAGE_VERSION`
  // so the boot marker in src/main.tsx can log "[boot] toaster v<version>" without
  // a runtime IPC roundtrip (which would defeat the purpose of a forensic anchor
  // that fires before any other module initializes).
  define: {
    "import.meta.env.PACKAGE_VERSION": JSON.stringify(pkg.version ?? "unknown"),
  },

  // Vitest configuration
  test: {
    environment: "happy-dom",
    globals: true,
    setupFiles: ["./src/test-setup.ts"],
    include: ["src/**/*.test.{ts,tsx}"],
  },

  // Multiple entry points
  build: {
    sourcemap: !!process.env.TAURI_DEBUG,
    rollupOptions: {
      input: {
        main: resolve(__dirname, "index.html"),
      },
      // NOTE: manualChunks intentionally NOT used.
      //
      // A previous config split node_modules into vendor-react / vendor-state /
      // vendor-tauri / vendor-icons / vendor (catch-all). That broke v0.1.0
      // with `Uncaught TypeError: Cannot set properties of undefined
      // (setting 'Children')` at boot — the catch-all `vendor` chunk inlined
      // `use-sync-external-store-shim` (a CJS-shaped React helper pulled in
      // by Zustand v5) and imported four named exports from the React chunk,
      // creating a CJS-interop race where React's IIFE ran against an
      // undefined exports object before the React chunk's body finished
      // initializing. Result: white screen on every fresh launch, no UI ever
      // mounted.
      //
      // Default Rollup chunking handles React + Zustand + i18next correctly,
      // and the resulting main bundle (~700 KB after gzip ~210 KB) is still
      // well under any meaningful budget for a desktop app. If chunk-splitting
      // is reintroduced, ALL React-touching modules — including
      // `use-sync-external-store`, every `react-*` package, `scheduler`, and
      // anything that imports React for hooks — must live in the same chunk
      // as `react` and `react-dom`, and a launch test against a real Tauri
      // bundle (not just `cargo tauri dev`) is mandatory.
    },
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
