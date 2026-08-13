# Fomalhaut Nocturne theme

Official React reference theme for Fomalhaut. The project uses React, Tailwind CSS,
shadcn/ui Luma, Zustand, and `fomalhaut-sdk`.

```sh
bun run build:theme:nocturne
```

The deployable theme is written to `themes/nocturne/dist`. Point
`[themes].default` in `/etc/fomalhaut/config.toml` at that absolute directory.

During `bun run dev`, a development-only transport presents the user-selection
screen and simulates PAM prompts in an ordinary browser. Use `fomalhaut` as the
fixture password. It also advertises simulated power actions so the complete
power menu can be previewed without invoking host or system power APIs.
Production builds never fall back to the simulated transport.
