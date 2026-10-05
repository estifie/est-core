# Security

`est-core` is a local-first shared library: it handles file paths,
process output, and in-memory secrets, and it never opens a network
listener or sends telemetry.

## Reporting

Do not open a public issue for a suspected vulnerability. Report it
through a [GitHub Security
Advisory](https://github.com/estifie/est-core/security/advisories/new)
(private), including what you ran and what you observed. Aim: first
response within three days, fix or mitigation plan within two weeks
for anything that leaks secret bytes.

## Supported versions

Pre-1.0: only the latest `main` is supported. After 1.0, the latest
minor of the current major.
