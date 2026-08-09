---
fomalhaut-lock: "patch:fix"
---

Stop remapping session-lock monitor windows through GTK after gtk4-session-lock has assigned and mapped them, preventing a GdkSurface segmentation fault during startup.
