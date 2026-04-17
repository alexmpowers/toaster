# Security Policy

## Reporting Vulnerabilities

If you discover a security vulnerability in Toaster, please report it responsibly. **Do not file a public GitHub issue.**

Instead, use one of these methods:

- **GitHub Security Advisories** (preferred): [Report a vulnerability](https://github.com/itsnotaboutthecell/toaster/security/advisories/new)
- **Email**: Contact the repo owner via their [GitHub profile](https://github.com/itsnotaboutthecell)

Please include:

- A description of the vulnerability
- Steps to reproduce
- Potential impact
- Suggested fix (if any)

We will acknowledge your report within 7 days and aim to provide a resolution timeline promptly.

## Supported Versions

| Version | Supported |
| ------- | --------- |
| 0.1.x   | ✅ Current (public preview) |
| < 0.1.0 | ❌ No longer supported |

Only the latest release receives security updates.

## Known Security Considerations

- **Local-only processing**: Toaster performs all transcription and media processing locally. No audio, video, or transcript data is transmitted to cloud or remote services.
- **macOS global shortcuts**: The macOS build uses private APIs for global keyboard shortcuts. This is a documented platform limitation.
- **FFmpeg subprocess**: FFmpeg is invoked as a subprocess for media processing and runs with the current user's permissions. It does not require or request elevated privileges.
- **Model files**: Transcription models are downloaded from configured sources and verified by hash before use.

## Scope

### In scope

- Vulnerabilities in the Toaster application (Rust backend or React frontend)
- Dependency vulnerabilities that affect Toaster
- Permission escalation or sandbox escape
- Unsafe handling of user-supplied media files

### Out of scope

- Social engineering attacks
- Attacks requiring physical access to the user's machine
- Vulnerabilities in third-party services or upstream dependencies not bundled with Toaster
- Denial-of-service attacks against local-only software
