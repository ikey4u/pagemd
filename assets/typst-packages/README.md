# Built-in Typst packages (source builds)

End users: Typst fenced blocks are supported at render time; package details live in `manifest.toml` and `build.rs`.

| Import | Use |
|--------|-----|
| `@preview/cetz:0.3.2` | Canvas drawing, charts |
| `@preview/fletcher:0.5.8` | Flowcharts, arrows |
| `@preview/codelst:2.0.2` | Code listings |

`manifest.toml` lists these for the build. `build.rs` downloads missing packages before compile; `rust-embed` then embeds `preview/` into the binary.

## Offline compile

```bash
PAGEMD_SKIP_TYPST_PACKAGES=1 cargo build
```

(requires `preview/{name}/{version}/` already present)
