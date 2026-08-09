---
fomalhaut-lock: "patch:fix"
---

Keep session-lock surfaces outside GtkApplication ownership and defer Rust cleanup until native destroy returns, avoiding gtk4-layer-shell 1.3.0 issue #122 on GTK 4.22 and newer.
