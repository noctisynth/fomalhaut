---
fomalhaut-pam: "patch:fix"
---

Send terminal PAM outcomes only after context cleanup so ordinary authentication rejection remains recoverable in the Web UI.

Classify bounded worker shutdown failures without exposing PAM or credential details.
