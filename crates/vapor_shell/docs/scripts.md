# Vapor command scripts

Command scripts expose repeatable REPL sequences through the script direct CLI
facade: `vapor script run NAME`.

Store source-controlled scripts at `.vapor/scripts/NAME.vapor`:

```text
# One normal Vapor command per line.
metadata
validate
docs build
root build
root publish --dry-run
```

Run or inspect them with:

```text
vapor script run NAME --dry-run
vapor script run NAME
```

Blank lines and comments are ignored. Parsing, validation, effects, help, and
exit status are identical to manually entered commands.

Scripts stop on the first error. They cannot invoke another script, exit the
host shell, authenticate Steam, perform a real publish, or apply IDE repairs.
Final publication is a manual interactive-shell action because Steam
authentication and upload confirmation must remain inside the human Vapor
session.
