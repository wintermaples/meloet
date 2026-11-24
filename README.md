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
  - Keep what you just played: play along or noodle without pressing record, then press 「さっきのを残す」 (Shift R) and it becomes a clip.
  - No MIDI keyboard at hand? Turn on 「キーで弾く」 (Ctrl K) and play with the letter keys: A S D F G H J K L ; are the white keys, W E T Y U O P the black ones, Z and X change the octave. It records like a keyboard.
  - Arpeggio: turn on 「アルペジオ」 for a track, hold a chord, and it plays as a phrase in time with the song (up, down, up and down, random; 1/4 to 1/32, triplets; 1 to 3 octaves; hold). One key repeats. What you hear is what gets recorded.
  - Metronome settings: volume, three sounds (beep, tick, cowbell), clicking only while recording, and tap tempo.
- Piano roll: write, move and quantize notes, velocity and CC lanes (sustain, pitch bend, modulation, …), scale fixing, chord stamps and the chord table of the key.
  - The keys of the scale can be colored (9 kinds: major, minor, harmonic and melodic minor, major and minor pentatonic, blues, dorian, mixolydian). With scale lock on, only those keys can be written.
  - Chord progressions: with the scale fixed, pick one of six common progressions (royal road, Canon, Komuro, …), listen to it, and place it one chord a bar.
  - Velocity tools for the selected notes: scale by a percent, ramp from one value to another, add a random spread; and "extend to the next note" (legato).
  - The notes of other tracks can be shown faintly behind the clip you edit (「ほかのトラックも見る」), to write a part that fits the others.
  - While a song plays, the notes under the play line light up in the track's color (「表示 ▸ 鳴っている音を光らせる」, on at first).
- Arrange: clips on tracks, splitting and joining clips, tempo and time signature changes, markers and automation.
  - Repeat clips: drag the handle at the right end of a clip's name, or use 「くり返す…」 (Ctrl R) and type how many times. The copies follow right after, as one undo step; several chosen clips repeat as one block.
  - Move whole sections: right-click a marker to copy its section right after itself, take the section out and close the gap, or put empty bars in before it. Clips, automation, tempo, time signatures and markers move together, as one undo step (the section must start and end on bar lines).
  - Duplicate a track (right-click its head): a copy right below with the same sound, effects, mixer settings, clips and automation. Plugins start from how they sound right now.
  - Turn a keys track, or some of its clips, into audio (right-click menu): it is rendered through its sound and effects onto a new audio track below, and the keys track is muted. Can be undone.
  - Drum patterns: the shelf has twelve one-bar drum patterns (8 beat, 16 beat, four on the floor, shuffle, fills…). Listen with ▶ and put one at the play line with Enter; press Enter again to add the next one after it.
- A mixer with nine built-in effects: 4-band EQ, compressor, reverb, delay, chorus, limiter, and now a gate (cuts noise between sounds), a de-esser (softens harsh "s" sounds of a voice) and saturation (warmth or grit).
  - Presets for the built-in effects: a few ready-made settings for each (for example "Even out vocals" on the compressor), and your own settings kept under a name on your PC.
  - Automation of the master volume, and one-step fade-outs: Edit ▸ add a fade-out at the end of the song, type the length in bars, and the master fader goes down to silence over the last bars. The line can be edited like any automation lane, and the exported file has the fade.
  - Finishing meters in the mixer, right of the master: the loudness of the last 3 seconds and of the whole play (LUFS), the true peak (dBTP), the master's sound in 24 bars, and 「モノで聞く」 to hear the song in mono (only for listening; the export is not changed).
  - A notice when the song is too loud (the master's limiter keeps holding it down, or the master goes over 0 dB), with a button that lowers every track at once.
  - A picture that moves with the sound: the master from low to high in 24 bars, left of 「マスター」 in the top band (View ▸ 「音の動きを見せる」 turns it off).
- 31 built-in sounds, VST3 and LV2 plugins, and SF2 files. Plugins run outside meloet, so a plugin that stops does not take meloet or your song with it.
  - Built-in sounds have four knobs in the parameters panel: brightness, attack, release and vibrato (drum kits: brightness and release). They can be automated like any parameter.
- Singing tracks: record audio, import audio files (wav, mp3, flac, ogg), stretch and pitch.
  - Comping: build one vocal from the best parts of several takes. Open 「いいとこ取り」 from a vocal clip's right-click menu and drag across a take to choose it for that part; the joins are cross-faded, and it can all be undone.
  - Pitch help for vocals: pull a sung clip toward the notes of a scale, from a light touch to all the way (0 to 100 %). For one voice or one-note-at-a-time instruments.
  - Reverse an audio clip, or normalize it so its loudest point is at −1.0 dB (right-click menu of the clip; both can be undone).
- Export the song to a wav, flac or mp3 file (mp3 at 128 to 320 kbps), as it plays or brought to the loudness music sites play at (−14 LUFS, true peak −1 dBTP). The check before export shows the loudness and the true peak.
  - The export can write the song's title, its artist and a picture (png or jpg) into the file (flac and mp3; wav takes the title and the artist). The artist is remembered.
  - When an export is done, a window shows the file's waveform, loudness and true peak, and can play the file in the PC's player or open its folder.
- File ▸ Recently opened songs: open one of the last 10 songs in one step (with its folder shown); the list can be cleared.
- Help ▸ Key list: every key by group, with a filter box. Help ▸ Open the user guide opens the user guide in your browser.
- Tells you when a new version of meloet is out (once a day at most, only the version is asked; can be turned off in the sound settings).
- The play, stop and record buttons and the fader of keyboards that send Mackie Control.
- Six color themes (dark and light, each also in sakura and lime).
- The screen in Japanese or English (View ▸ ことば / Language; at first it follows the PC's language). New songs and tracks get their names in the language they are made in. The user guide is in both: [Japanese](docs/user/README.ja.md), [English](docs/user/README.en.md).
- An AI link: AI apps such as Claude can read the song, give advice and edit it when asked (see [AI link](#ai-link)).

## Supported environment and install

### Supported environment

- Ubuntu 24.04 or 26.04 (amd64), with PipeWire for sound.
- A USB MIDI keyboard. Without one, you can still write notes with the mouse.
- An internet connection the first time, to download the sound pack (about 240 MB).

### Install

1. Download `meloet_0.3.0_amd64.deb` from the [Releases page](https://github.com/wintermaples/meloet/releases). If there is no deb there yet, you can build one yourself ([Build the deb](docs/development/ubuntu.md#build-the-deb)).
2. Open a terminal where the file is, and type:

   ```bash
   sudo apt install ./meloet_0.3.0_amd64.deb
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
