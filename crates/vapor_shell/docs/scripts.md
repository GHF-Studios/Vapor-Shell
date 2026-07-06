# Vapor command scripts

Command scripts expose repeatable REPL sequences through the one-shot CLI without
introducing an ambient Bash or PowerShell execution surface.

Store source-controlled scripts at `.vapor/scripts/NAME.vapor`:

```text
# One normal Vapor command per line.
docs build
self stage
self smoke
steam publish --account vapor-builder --plan
```

Run or inspect them with:

```text
vapor script run NAME --plan
vapor script run NAME
```

Blank lines and comments are ignored. Parsing, validation, effects, confirmation,
help, and exit status are identical to manually entered commands. Scripts stop on
the first error, cannot invoke another script, and cannot use `exit` to terminate
an interactive host.
