# Applet instances share a state file, not a service

The panel runs one applet instance per monitor and none of them owns the timer, so
they share the phase through a small file under `$XDG_RUNTIME_DIR` (plus
`app/<id>` under Flatpak) and ring each notification through a one-shot marker file
next to it. A D-Bus service or a socket would do the same job and were rejected: they
cost a daemon, a name and a lifecycle, and the file already survives a panel restart,
clears on logout, and needs no cleanup code. The cost of the file is that reads happen
by polling once a second, so watch the wakeups before reaching for inotify.
