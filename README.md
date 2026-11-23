<p align="center"><img src="crates/meloet-app/icons/128x128.png" width="128" alt="meloet logo"></p>

<h1 align="center">meloet</h1>

<p align="center">A DAW for people who have just bought a MIDI keyboard.</p>

<p align="center">English | <a href="README.ja.md">日本語</a></p>

<p align="center"><img src="docs/screenshots/app/readmeApp-1280x800-100.png" width="800" alt="meloet: the arrange and the piano roll of a song with six tracks"></p>

## Overview

meloet is a music app (a DAW) for people who have just bought their first MIDI keyboard. Plug the keyboard in by USB, choose where the sound comes out, and you can play, record and edit notes. Recorded keys are placed where you heard them, and every change can be undone step by step.

- The screens are in Japanese only for now.
- meloet is still being developed.

### Main features

- Record from a USB MIDI keyboard: count-in, loop recording into takes, and overdub (「重ねる」) that adds to the take on each pass.
- Piano roll: write, move and quantize notes, velocity and CC lanes (sustain, pitch bend, modulation, …), scale fixing, chord stamps and the chord table of the key.
- Arrange: clips on tracks, splitting and joining clips, tempo and time signature changes, markers and automation.
- A mixer with six built-in effects (4-band EQ, compressor, reverb, delay, chorus, limiter).
- 31 built-in sounds, VST3 and LV2 plugins, and SF2 files. Plugins run outside meloet, so a plugin that stops does not take meloet or your song with it.
- Singing tracks: record audio, import audio files (wav, mp3, flac, ogg), stretch and pitch.
- Export the song to a wav file.
- The play, stop and record buttons and the fader of keyboards that send Mackie Control.
- Six color themes (dark and light, each also in pink and mint).
- An AI link: AI apps such as Claude can read the song, give advice and edit it when asked (see [AI link](#ai-link)).

## Supported environment and install

### Supported environment

- Ubuntu 24.04 or 26.04 (amd64), with PipeWire for sound.
- A USB MIDI keyboard. Without one, you can still write notes with the mouse.
- An internet connection the first time, to download the sound pack (about 240 MB).

### Install

1. Download `meloet_0.2.1_amd64.deb` from the [Releases page](https://github.com/wintermaples/meloet/releases). If there is no deb there yet, you can build one yourself ([Build the deb](docs/development/ubuntu.md#build-the-deb)).
2. Open a terminal where the file is, and type:

   ```bash
   sudo apt install ./meloet_0.2.1_amd64.deb
   ```

   Do not forget `./`. The parts meloet needs (WebKitGTK, ALSA, PipeWire, …) are installed with it.

3. Open 「meloet」 from the list of apps, or type `meloet` in a terminal.
4. On the first start, meloet asks to download the sound pack (「取りにいく」 starts it). You can play while it downloads; sounds that have not arrived yet play as a sine wave.

To remove meloet, type `sudo apt remove meloet`. Settings and the sound pack stay in your home folder; where they are and how to delete them is in 「消しかた」 of the [user guide](docs/user/README.ja.md) (Japanese).

## Quick start

1. **Connect the keyboard.** Plug it in by USB, before or after starting meloet. The first time, meloet asks 「音をどこから鳴らしますか」 (where should the sound come out): choose with <kbd>↑</kbd> <kbd>↓</kbd> and press <kbd>Enter</kbd>. You can change it later in the sound settings (<kbd>Ctrl</kbd> <kbd>,</kbd>).
2. **Choose a sound.** Open the shelf (「棚」, <kbd>P</kbd>). Choosing a row with <kbd>↑</kbd> <kbd>↓</kbd> lets you hear it; <kbd>Enter</kbd> puts it on the selected track.
3. **Record.** Press <kbd>R</kbd> (or the ● button) to record to the selected track, and <kbd>Space</kbd> to stop. <kbd>L</kbd> turns the loop on; each pass of the loop becomes a take, or, with 「重ねる」 on, is added to the same take. <kbd>C</kbd> turns the metronome on and off.
4. **Edit notes.** <kbd>E</kbd> opens the piano roll. Tools: 「選ぶ」 select (<kbd>1</kbd>), 「書く」 draw (<kbd>2</kbd>), 「消す」 erase (<kbd>3</kbd>). <kbd>Q</kbd> quantizes the chosen notes.
5. **Mix.** <kbd>M</kbd> opens the mixer.
6. **Undo.** <kbd>Ctrl</kbd> <kbd>Z</kbd> undoes and <kbd>Ctrl</kbd> <kbd>Shift</kbd> <kbd>Z</kbd> redoes. <kbd>H</kbd> shows the undo history.
7. **Save and export.** <kbd>Ctrl</kbd> <kbd>S</kbd> saves the song and <kbd>Ctrl</kbd> <kbd>Shift</kbd> <kbd>E</kbd> exports it to a wav file.

- All keys: [`docs/spec/4-keys.md`](docs/spec/4-keys.md).
- Keyboards, the sound pack, plugins and what to do when there is no sound: the [user guide](docs/user/README.ja.md) (Japanese).

## AI link

AI apps that speak MCP (such as Claude Code and Claude Desktop) can use the running meloet. The AI can:

- read the song and give advice;
- write to the song when you ask (notes, mixer, effects, sounds, automation, tracks and clips, save, open, export). Each change is one step in the undo history, so you can undo it;
- check exported audio and tell you numbers such as loudness. It can also export a single note to learn what a sound is like.

### Turn it on and register

1. In meloet, turn on 「音の設定 ▸ くわしい設定 ▸ AI とつなぐ」 (off at first). Only apps run by the same user on this PC can connect; nobody else, and nothing on the internet.
2. Register meloet in the AI app. For Claude Code, type this once:

   ```bash
   claude mcp add --transport stdio --scope user meloet -- /usr/bin/meloet-mcp
   ```

   For Claude Desktop, add this to `~/.config/Claude/claude_desktop_config.json` and restart Claude Desktop (if something is already there, add the `meloet` line inside `mcpServers`):

   ```json
   {"mcpServers": {"meloet": {"command": "/usr/bin/meloet-mcp"}}}
   ```

### Please read before using it

- **Use the AI as a tool for your own music.** We do not recommend music made by the AI alone. meloet is a tool for you to make music; use the AI for advice and for edits that take a lot of work.
- **Your music data is sent.** What the AI reads from your song (notes, tempo, mixer settings, numbers from exported audio, …) is sent through the AI app to the servers of the company that runs the AI. What is sent and how it is used depends on the terms of that AI app. Turn the AI link on only if that is fine with you.
- Audio files exported for the AI are put in a temporary folder that only your user can read (`/tmp/meloet-ai-<user number>`). They are deleted after one hour, when you turn the AI link off, or when meloet ends.
- The AI cannot change the song while you are recording or exporting, and it does not open another song while there are unsaved changes.

Details: 「AI とつなぐ」 in the [user guide](docs/user/README.ja.md) (Japanese).

## Development

Development is supported on Ubuntu 24.04 or later only, for now.

```bash
(cd ui && npm ci && npm run build)
cargo run --release -p meloet-app
scripts/check-all.sh
```

The setup, the checks, building the deb and the development tools are in [`docs/development/`](docs/development/README.md) ([Ubuntu](docs/development/ubuntu.md)).

## Bug reports and contact

Please write bugs and questions in [GitHub Issues](https://github.com/wintermaples/meloet/issues). It helps if you include:

- the version of meloet (「ヘルプ ▸ meloet について」) and of Ubuntu;
- what you did, what you expected, and what happened;
- the log: start meloet from a terminal by typing `meloet`, and copy the text it prints there.

## License

The code is under either of these, at your choice:

- MIT ([`LICENSE-MIT`](LICENSE-MIT))
- Apache License 2.0 ([`LICENSE-APACHE`](LICENSE-APACHE))

The font IBM Plex is under OFL-1.1. The sound material is CC0, CC BY or MIT. The credits for the sound pack and the parts are in [`NOTICE`](NOTICE) and in 「meloet について」 in the app.

## Disclaimer

- meloet is provided "as is", without warranty of any kind, as written in the licenses above. The authors are not responsible for any damage from using it, including lost songs or recordings. Please keep copies of songs that matter to you.
- Plugins, SF2 files and AI apps that you use with meloet are made by others, under their own terms. Their behavior and how they handle your data are the responsibility of the people who provide them.
- Advice and edits from an AI can be wrong. Check them yourself.
- Company and product names in this document are trademarks or registered trademarks of their owners.
