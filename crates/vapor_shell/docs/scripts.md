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
content validate
content publish ghf-studios/loo-cast/loo-cast-packagepack --dry-run
```

Run or inspect them with:

```text
vapor script run NAME --dry-run
vapor script run NAME
```

Blank lines and comments are ignored. Parsing, validation, effects, help, and
exit status are identical to manually entered commands.

Scripts stop on the first error. They cannot invoke another script, exit the
host shell, authenticate Steam, perform a real root or content publish, create
or delete real Workshop authority, or apply IDE repairs. They may run local
content package/acquire/install/update/verify/repair/uninstall workflows and
publication dry-runs. Final root, registry, or Workshop publication is a manual
interactive-shell action because Steam authentication, authority mutation, and
upload confirmation must remain inside the human Vapor session.
