# SR35 MTP tools

Small libmtp programs to put the HomeFi APK on the Astell&Kern SR35, which has
no adb and rejects the per-folder MTP queries `mtp-sendfile` uses. See
`docs/HANDOFF.md` §7 for the whole install route.

```bash
brew install libmtp
for t in mtpswap mtpput mtpls mtpget; do clang -o $t $t.c -I$(brew --prefix)/include -L$(brew --prefix)/lib -lmtp; done
```

| Tool | Use |
| --- | --- |
| `mtpswap <folder id> <name> <backup> <new>` | back up a file, check its size, delete it, upload the new one |
| `mtpput <file> <folder> <name>` | upload a file into a folder of internal storage |
| `mtpls <folder id>` | list a folder |
| `mtpget <out dir> <name>...` | download files by name |
