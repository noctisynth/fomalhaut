# Fomalhaut React theme

Official React reference theme for Fomalhaut. The project uses React, Tailwind CSS,
shadcn/ui Luma, Zustand, and `fomalhaut-sdk`.

```sh
bun run build:theme
```

The deployable theme is written to `packages/fomalhaut-theme/dist`. Point
`[frontend].path` in `/etc/fomalhaut/config.toml` at that absolute directory.

During `bun run dev`, a development-only transport presents the user-selection
screen and simulates PAM prompts in an ordinary browser. Use `fomalhaut` as the
fixture password. Production builds never fall back to the simulated transport.
