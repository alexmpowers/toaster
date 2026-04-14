---
description: "Use when editing Qt frontend code — enforces signal safety, naming conventions, and the libtoaster boundary."
applyTo: "frontend/**"
---
# Frontend Conventions

## Signal re-entrancy guard

**Always** wrap programmatic content changes in `blockSignals`:

```cpp
m_textView->blockSignals(true);
m_textView->setHtml(html);
m_textView->blockSignals(false);
```

Without this, `setHtml()` fires `cursorPositionChanged` → handler calls `buildHtml()` → infinite recursion / stack overflow.

## Naming

- Member variables: `m_` prefix (`m_player`, `m_transcript`)
- Methods: camelCase (`onPlayClicked`, `buildHtml`)
- Signals/slots: Qt naming (`clicked`, `valueChanged`)

## Architecture boundary

- Frontend code consumes `libtoaster` via its C API (`toaster.h`)
- Never add Qt includes or types into `libtoaster/`
- Never call FFmpeg or plugin internals directly — go through the registered plugin API
