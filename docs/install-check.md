# Testing the deb on a real PC (steps for the owner)

In a container we can only see that the deb installs, removes, and finds all its shared libraries (after `scripts/build-deb.sh`, the lead or a helper checks this in plain `ubuntu:24.04` and `ubuntu:26.04` containers). The screen, the sound, the keyboard and the plugin windows are checked on a real PC or a VM.

**Where**: each of Ubuntu 24.04 and Ubuntu 26.04 (a real PC or a VM). Use a new user who has never installed meloet (no `~/.config/meloet` and so on). In a VM, pass the sound output (headphones or speakers) and the USB keyboard to the VM.

**What to take**: `target/deb/meloet_0.4.0_amd64.deb`.

At each step, for anything that did not go as expected, keep a picture of the screen and the text shown in the terminal (step 2 below).

## 1. Install

```bash
sudo apt install ./meloet_0.4.0_amd64.deb
```

- [ ] It installs together with the parts it depends on, and ends with no errors.
- [ ] `ls -l /usr/bin/meloet /usr/bin/meloet-plughost` shows both.
- [ ] `ls /usr/share/doc/meloet/` has `LICENSE-MIT`, `LICENSE-APACHE` and `NOTICE`.
- [ ] 「meloet」 is in the list of apps (the icon is a white 「m」 and a yellow note on a black square with round corners).

## 2. Open

Open it once from the list of apps, and once from the terminal with `meloet`. When it is from the terminal, keep the text it prints.

- [ ] The window opens and the screen is drawn (not all white, not all black, and it does not fall over at once).
- [ ] The arrange, the input meter and the meters move (the WebGL faces are drawn).
- [ ] 「音の素材（約 240 MB）を取りにいきます」 is shown.

### 2-1. Audio thread priority (when started from the terminal)

When you start it from the terminal with `meloet`, one of these lines is printed in the first few seconds. Tell us which one it was.

- `meloet: the audio output thread runs at real-time priority 20 (limit)` or `(rtkit)`: the audio thread runs at real-time priority.
- `meloet: no real-time priority for the audio output thread: …`: the priority could not be raised (there is sound, but it breaks up more easily on a busy PC).

## 3. Connect the keyboard

- [ ] Before the keyboard is connected, the input meter shows 「鍵盤を USB でつなぐと、ここに出ます。つながなくても、マウスで音を書けます。」.
- [ ] When the USB keyboard is plugged in, its name is shown in the input meter.
- [ ] 「音をどこから鳴らしますか」 and the list of outputs are shown. The test sound of a row plays. Enter decides.
- [ ] Pressing a key lights the keyboard picture in the input meter. At this point it still plays a sine wave.
- [ ] Write down the buffer number on the status line (24.04 and 26.04 have different versions of PipeWire).

## 4. Download the sound pack

- [ ] Pressing 「取りにいく」 shows the progress (% and MB).
- [ ] You can play the keyboard while it downloads (sounds not yet here play a sine wave).
- [ ] The grand piano arrives first, and 「グランドピアノ が鳴るようになりました ・ のこりの音色を取っています」 is shown.
- [ ] The first track plays with the grand piano sound.
- [ ] At the end, 「音の素材をぜんぶ取りました。31 の音色が鳴ります。」.
- [ ] `du -sh ~/.local/share/meloet/sounds` is about 240 MB.
- [ ] (If you can) cut the network in the middle, connect again, press 「もう一度」, and the download goes on from where it stopped.

## 5. Plugins

First install one VST3 plugin (for example, `sudo apt install surge-xt` may not be in the Ubuntu archive, so use a deb such as LSP, or something that installs into `/usr/lib/vst3`).

- [ ] The plugins in `/usr/lib/vst3` are shown under 「あとから入れたもの」 on the shelf (P) (the first scan can take tens of seconds).
- [ ] Add the plugin: it plays from the keyboard (an instrument) or changes the sound (an effect).
- [ ] The plugin window opens.
- [ ] `ps -ef | grep meloet-plughost` shows the process that runs the plugin.

## 6. Save and open

- [ ] Record a little, press Ctrl+S, close meloet, open it again, open the song, and it plays with the same sound.

### 6-1. Song files in the file manager

Use the song saved in step 6 (`~/ミュージック/<song name>/<song name>.meloet`, or `~/Music/…`).

- [ ] `xdg-mime query filetype <the song file>` prints `application/x-meloet`.
- [ ] `xdg-mime query default application/x-meloet` prints `meloet.desktop`.
- [ ] In the file manager (Files), the song file shows the meloet icon, and its type (right click ▸ Properties) is 「meloet の曲」 (`meloet song` on an English desktop).
- [ ] Right click ▸ 「別のアプリケーションで開く」 (Open With) lists meloet first.
- [ ] With meloet closed, double-click the song file: meloet starts and the song opens (its name is at the top left, and 「保存済み」).
- [ ] With meloet open on another song that has no unsaved changes, double-click the song file: no second window opens, the song opens in the meloet that runs, and its window comes to the front. Write down whether it came to the front, or only said that it is ready (some desktops do that).
- [ ] Change something (for example the tempo), then double-click the song file again: 「保存していない変更があります」 is asked first, as when you open a song from the menu.
- [ ] Double-click the song that is open now: nothing changes, and the window comes to the front.
- [ ] From the terminal, `meloet <the song file>` while meloet runs: the command ends at once, and the song opens in the meloet that runs.
- [ ] Rename a copy of a text file to `broken.meloet` and double-click it: meloet says it cannot open it, as when you open it from the menu.

## 7. Remove

```bash
sudo apt remove meloet
```

- [ ] It ends with no errors. `/usr/bin/meloet` and `/usr/bin/meloet-plughost` are gone.
- [ ] It is gone from the list of apps.
- [ ] `ls /usr/share/mime/packages/meloet.xml /usr/share/applications/meloet.desktop` finds neither.
- [ ] `xdg-mime query filetype <the song file>` no longer prints `application/x-meloet` (a JSON or text type instead), and `xdg-mime query default application/x-meloet` prints nothing.
- [ ] In the file manager the song file no longer shows the meloet icon, and double-clicking it does not start meloet.
- [ ] `~/.config/meloet`, `~/.local/share/meloet` and so on stay (they belong to the user, so it is right that they stay).

## Sending the results

For each of 24.04 and 26.04, send the checked items above, what was different, the text printed in the terminal, the buffer number on the status line, and the kind of PC (real or VM; graphics: NVIDIA, AMD or Intel; X11 or Wayland).
