# Fomalhaut Nocturne theme

Official React reference theme for Fomalhaut. The project uses React, Tailwind CSS,
shadcn/ui Luma, Zustand, and `fomalhaut-sdk`.

```sh
bun run build:theme:nocturne
```

The deployable theme is written to `themes/nocturne/dist`. During local
development, point `[themes].default` in `/etc/fomalhaut/config.toml` at that
absolute directory. Arch users can instead install `fomalhaut-theme-nocturne`
from the AUR and select its stable ID with `default = "nocturne"`; the AUR build
uses npm so it does not depend on the project's Bun canary toolchain.

During `bun run dev`, a development-only transport presents the user-selection
screen and simulates PAM prompts in an ordinary browser. Use `fomalhaut` as the
fixture password. It also advertises simulated power actions so the complete
power menu can be previewed without invoking host or system power APIs.
Production builds never fall back to the simulated transport.
