# Changelog

All notable changes, newest first. Format: Keep a Changelog;
versions: SemVer.

## [Unreleased]

## [0.1.0] - 2026-10-06

### Added

- `cli`: exit codes, versioned JSON envelope (`ok`/`err`), string
  escaper, stderr errors, and the destructive-command `Confirm` rule.
- `paths`: layered base resolution, product config dirs, pointer
  files.
- `secret`: `Secret<T>` redaction wrapper, log `redact`, reveal audit
  log.
- `config`: `[core]` settings loader with warnings, duration syntax.
- `process`: supervised spawn with timeout, output cap, combined log.
- `store`: atomic writes, mode-correct dirs, file locks, retention.
- `log`: `EST_LOG` levels, structured stderr records.
