# Vapor command scripts

Command scripts expose repeatable REPL sequences through the script direct CLI
facade: `vapor script run NAME`. Startup scripts use the same files and grammar
through `vapor --startup-script NAME`, then keep the interactive shell open.

Store source-controlled scripts at
`resources/vapor/vapor-scripts/NAME.vapor`. Runtime app scripts can also be
shipped in the installed app's `resources/vapor/vapor-scripts/` directory for
Steam launch entries that must work before any source is open:

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
vapor --startup-script NAME
```

Blank lines and comments are ignored. Parsing, validation, effects, help, and
exit status are identical to manually entered commands.

Scripts stop on the first error. They cannot invoke another script, exit the
host shell, perform a real root or content publish, delete real Workshop
authority, or apply IDE repairs. They may run local content package/acquire/
install/update/verify/repair/uninstall workflows and publication dry-runs.
Workshop acquisition may use SteamCMD authentication because startup scripts run
inside the visible Vapor shell session. Final root, registry, or Workshop
publication is a manual interactive-shell action because authority mutation and
upload confirmation must remain inside the human Vapor session.
