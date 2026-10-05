# EST CLI contract v1

Every EST binary speaks the same surface, for people and agents alike.
Implemented by `est_core::cli`; locked by `tests/contract.rs`.

## Exit codes

- `0` ok — done, as asked.
- `1` failed — tried and failed (missing secret, bad config).
- `2` wrong usage — bad flags, unknown command, missing argument.

`run`-style wrappers exit with the child's own status instead.

## JSON envelope

`--json` rides on any command, anywhere on the line, and prints one
object on stdout:

```json
{"ok": true, "v": 1, "secrets": 26}
{"ok": false, "v": 1, "error": "no job \"x\""}
```

- `"v": 1` is the contract version. Consumers must ignore unknown
  fields; producers must never remove `ok` or `v`.
- Error messages go inside the envelope on stdout (exit 1); usage
  errors stay human text on stderr (exit 2).
- Three shapes stay raw bytes either way, because no envelope belongs
  around bytes: secret values, log content, and document templates.

## Text mode

- Data goes to stdout; errors go to stderr as `prog: message`.
- Destructive commands ask first — unless `--yes` is passed, or stdin
  is not a terminal, in which case they refuse rather than guess.
  Agents always pass `--yes`.

## Escaping

Envelope strings escape `"`, `\`, and control characters (`\n`,
`\r`, `\t` short, the rest `\u00xx`); non-ASCII passes through
untouched.
