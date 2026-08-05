

<p align="center">
  <img src="src/assets/toaster.png" alt="Toaster" width="200" />
</p>

<p align="center">
  <img src="src/assets/toaster_text.svg" alt="Toaster" width="200" />
</p>

<p align="center">
  <strong>Ayudándote a sonar nítido.</strong><br/>
  Edita video editando texto: totalmente en tu máquina.
</p>

<p align="center">
  <a href="https://github.com/alexmpowers/toaster/releases"><img src="https://img.shields.io/github/v/release/alexmpowers/toaster?include_prereleases&label=latest&style=flat-square" alt="Latest release" /></a>
  <a href="LICENSE"><img src="https://img.shields.io/github/license/alexmpowers/toaster?style=flat-square" alt="License" /></a>
  <a href="https://github.com/alexmpowers/toaster/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/alexmpowers/toaster/ci.yml?branch=main&style=flat-square&label=ci" alt="CI status" /></a>
</p>

<p align="center">
  <a href="#features">Características</a> ·
  <a href="#how-it-works">Cómo funciona</a> ·
  <a href="#quick-start">Inicio rápido</a> ·
  <a href="#contributing">Contribuir</a>
</p>

---

## ¿Por qué Toaster?

Grabarte a ti mismo es fácil. ¿Eliminar cada "eh", falso inicio y pausa incómoda? Ahí está la parte difícil.

Toaster es un editor de escritorio **centrado en la transcripción** para audio y video hablado. En lugar de desplazarte por una línea de tiempo, lees tus palabras, seleccionas las que no quieres y las eliminas, como si editaras un documento. Toaster se encarga del corte de audio, la sincronización de la forma de onda y la exportación de subtítulos detrás de escena.

Todo se ejecuta localmente. Sin APIs en la nube, sin cargas de archivos, sin suscripciones.

## Características

- **Edita multimedia editando texto** — visualiza tu transcripción, selecciona palabras y elimínalas/silencia/restaúralas con un clic
- **Transcripción local** — genera transcripciones a nivel de palabra con modelos en el dispositivo (ecosistema Whisper)
- **Detección de palabras de relleno y disfluencias** — resalta automáticamente "eh", "um", "saben" y pausas
- **Edición no destructiva** — cada acción es reversible; tu archivo original nunca se modifica
- **Reproducción sincronizada** — la transcripción, la forma de onda y el video permanecen sincronizados mientras editas
- **Exporta multimedia limpia** — renderiza tu corte final con subtítulos (SRT/VTT) y texto del guion
- **Guardar y reanudar** — los archivos de proyecto conservan tus ediciones para sesiones iterativas
- **Privacidad ante todo** — sin llamadas de red en tiempo de ejecución, sin telemetría, totalmente fuera de línea

## Cómo funciona

1. **Abre** un archivo de video o audio
2. **Transcribe** con un modelo local — Toaster genera una transcripción a nivel de palabra
3. **Lee y edita** — selecciona las palabras que quieres eliminar y presiona Delete
4. **Vista previa** — reproduce tu edición en tiempo real con la forma de onda y el video sincronizados
5. **Exporta** — renderiza la multimedia limpia junto con los subtítulos y el guion

Todo el flujo de trabajo se mantiene en tu máquina. Tu multimedia nunca sale de tu computadora.

## Inicio rápido

### Instalar desde la versión publicada

Descarga el instalador más reciente desde la página de [Versiones](https://github.com/alexmpowers/toaster/releases).

| Plataforma       | Arquitecturas | Formato      |
| -------------- | ------------- | ----------- |
| Windows        | x64, ARM64    | `.msi`      |
| Linux (Debian) | x64, ARM64    | `.deb`      |
| Linux (RPM)    | x64, ARM64    | `.rpm`      |
| Linux (any)    | x64, ARM64    | `.AppImage` |

> **Nota:** Los instaladores de Windows en v0.1.0 no están firmados; SmartScreen mostrará "Windows protegió tu PC" la primera vez. Haz clic en **Más información → Ejecutar de todos modos** para instalar. La firma de código está planificada para una versión posterior.
>
> Las compilaciones para macOS no se publican actualmente. Compila desde el código fuente si necesitas una aplicación para macOS: consulta [docs/build.md](docs/build.md).

### Compilar desde el código fuente

Consulta [docs/build.md](docs/build.md) para la configuración completa de la plataforma. La versión corta:

```bash
bun install --frozen-lockfile
cargo tauri dev
```

En Windows, ejecuta `.\scripts\setup-env.ps1` primero para configurar el entorno de compilación MSVC + LLVM.

### Solución de problemas

**"localhost refused to connect" al iniciar (Windows)**

Si la aplicación instalada muestra una página de `localhost refused to connect` en lugar del editor, es muy probable que tengas una **compilación de depuración** residual de `toaster.exe` aún en ejecución de una sesión anterior de `cargo tauri dev` / `cargo tauri build --debug`. El complemento de instancia única de Tauri reenvía cada lanzamiento desde el menú Inicio a esa ventana inactiva, la cual espera un servidor de desarrollo Vite en `http://localhost:1420` y muestra el error de conexión de WebView2 cuando este no responde.

```powershell
Get-Process toaster | Stop-Process -Force
Start-Process "C:\Program Files\Toaster\toaster.exe"
```

El MSI publicado desde la página de Versiones incluye todos los activos del frontend directamente en `toaster.exe` y nunca contacta a `localhost:1420`.

## Pila tecnológica

| Capa          | Tecnología                                |
| ------------- | ----------------------------------------- |
| Carcasa de escritorio | [Tauri 2.x](https://tauri.app/)           |
| Backend       | Rust                                      |
| Frontend      | React · TypeScript · Tailwind CSS         |
| Estado        | Zustand                                   |
| Transcripción | Inferencia con modelo local (ecosistema Whisper) |
| Exportación   | FFmpeg 7                                  |

## Contribuir

¡Aceptamos contribuciones! Por favor, lee [CONTRIBUTING.md](CONTRIBUTING.md) antes de abrir un PR.

```bash
# Run the checks contributors are expected to pass
cd src-tauri && cargo test && cargo clippy
npm run lint
```

Para contribuciones de traducción, consulta [CONTRIBUTING_TRANSLATIONS.md](CONTRIBUTING_TRANSLATIONS.md).

## Agradecimientos

Toaster es un fork de [Handy](https://github.com/cjpais/Handy) de [CJ Pais](https://github.com/cjpais). Handy demostró que una herramienta de voz gratuita, de código abierto y totalmente fuera de línea podría ser simple, privada y impulsada por la comunidad. Toaster se construye sobre esa base con un flujo de trabajo de edición centrado en la transcripción.

Estamos agradecidos con los proyectos que hacen posible Toaster:

- [Tauri](https://tauri.app/) — el marco de aplicaciones nativo en Rust que mantiene el paquete pequeño y el tiempo de ejecución rápido
- [Whisper](https://github.com/openai/whisper) de OpenAI — el modelo de reconocimiento de voz en el corazón de la transcripción local
- [whisper.cpp](https://github.com/ggerganov/whisper.cpp) y [ggml](https://github.com/ggerganov/ggml) — inferencia multiplataforma y aceleración por hardware
- [FFmpeg](https://ffmpeg.org/) — el cuchillo suizo del procesamiento multimedia

## Licencia

MIT: consulta [LICENSE](LICENSE) para más detalles.
