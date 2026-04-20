# Contributing Translations to Toaster

Thanks for helping improve Toaster localization.

## Quick start

1. Fork the repository
2. Copy the English translation file
3. Translate values (not keys)
4. Register language metadata
5. Open a pull request

## Translation files

```text
src/i18n/locales/
  en/translation.json          # source
  <language-code>/translation.json
```

## Add a new language

### 1) Create a language folder

Use an ISO 639-1 code (for example: `de`, `es`, `ja`, `ko`, `pt`).

### 2) Copy the English source

```bash
cp src/i18n/locales/en/translation.json src/i18n/locales/<language-code>/translation.json
```

PowerShell equivalent:

```powershell
Copy-Item src\i18n\locales\en\translation.json src\i18n\locales\<language-code>\translation.json
```

### 3) Translate values only

- Keep keys unchanged
- Preserve placeholders like `{{error}}` or `{{model}}`
- Keep JSON structure valid

### 4) Register language metadata

Update `src/i18n/languages.ts` with language name + native name.

### 5) Test in app

1. Run app (`npm run tauri dev` or `cargo tauri dev`; on Windows run `.\scripts\setup-env.ps1` first)
2. Open language settings
3. Select your language
4. Verify labels/flows

## Translation guidelines

Do:

- Use natural, concise phrasing
- Keep product terminology consistent
- Preserve variables exactly

Do not:

- Translate placeholders (`{{...}}`)
- Change key names
- Break JSON formatting

## Pull request notes

- Include language name in PR title (for example: `docs(i18n): add German translation`)
- Mention whether this is a new language or corrections to an existing one
- Add screenshots if any text overflows or truncation changes were needed
