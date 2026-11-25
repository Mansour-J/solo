# Installation

Solo is a desktop app for one person. Download the installer for Windows, macOS or Linux from the
release page and open the app. That's all: it keeps your books on your computer, with its own
PostgreSQL and API running inside the app ([ADR 0014](../decisions/0014-local-first-desktop.md)).
There's no account to create and no sign-in ([ADR 0016](../decisions/0016-one-person-no-sign-in.md)).
Nothing is sent anywhere, and it works offline.

- **Your data** lives in the app's local data folder: on Windows `%LOCALAPPDATA%\<identifier>\local`,
  on macOS `~/Library/Application Support/<identifier>/local`, on Linux `~/.local/share/<identifier>/local`.
  Anyone who can use your computer account can open it, as with any file: protect the account, and
  turn on disk encryption.
- **Documents** are downloaded as PDFs to send from your own email: the app has no mail server.
- **A copy of everything:** Settings › Your data › Download data.
- **Upgrading:** install the new version over the old one, or use Settings › Updates. Your books are
  kept and their database is updated when the new version starts.

## Ubuntu and Debian

```bash
sudo apt install ./Solo_<version>_amd64.deb
```

Or make the `.AppImage` executable (`chmod +x`) and run it.

## Troubleshooting

| Symptom | Cause and fix |
|---|---|
| "Your books couldn’t be opened" at start-up | The local database didn't start; the message says why. **Restart and try again**. If it persists, check free disk space and that no other copy of the app is running |
| A blank window on Linux | The app sets `WEBKIT_DISABLE_DMABUF_RENDERER=1` itself; if you launch it some other way, set it too |
