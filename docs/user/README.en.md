# meloet for the people who use it

meloet is a DAW for people who have just bought a MIDI keyboard. This document guides you from installing meloet, through using it, to removing it (documents for developers are in `docs/development/`). The Japanese version, [README.ja.md](README.ja.md), is the source; this one follows it.

The current version is 0.5.0. It runs on Ubuntu 24.04 and 26.04.

## Installing

1. Download `meloet_0.5.0_amd64.deb` from the GitHub Releases page.
2. Open a terminal where the downloaded file is, and type:

   ```bash
   sudo apt install ./meloet_0.5.0_amd64.deb
   ```

   Do not forget the `./`. The parts meloet uses (WebKitGTK, ALSA, PipeWire and so on) are installed with it.

3. "meloet" appears in the list of apps. Typing `meloet` in a terminal starts it too.

What is installed:

| Place | What it is |
|---|---|
| `/usr/bin/meloet` | meloet itself |
| `/usr/bin/meloet-plughost` | Runs plugins (meloet starts it itself; you do not start it directly) |
| `/usr/bin/meloet-mcp` | The bridge between AI apps and meloet (the AI app starts it; you do not start it directly. See "Link to AI") |
| `/usr/share/applications/meloet.desktop` | Puts meloet in the list of apps |
| `/usr/share/doc/meloet/` | The licenses (`LICENSE-MIT`, `LICENSE-APACHE`) and the credits of the parts it uses (`NOTICE`) |

## Connecting a keyboard

1. Connect the MIDI keyboard by USB. Before or after starting meloet, either is fine.
2. While there is no keyboard, the input meter says "Plug in a USB keyboard and it shows here. Or write with the mouse."
3. When it is connected, its name shows in the input meter. The first time, meloet asks "Where should the sound come out?" with a list of outputs. Choose with ↑ ↓ and press Enter. The test tone on each row lets you check where the sound comes out.
4. meloet remembers the chosen output and does not ask again. You can change it later in "Sound settings" (Ctrl+,).

## The sound pack

The sounds of 31 instruments such as the grand piano are not inside meloet. The first time it starts, meloet fetches them from the internet (about 240 MB).

- At the start meloet says "Fetch the sound pack (about 240 MB)". Press "Fetch" to start. Press "Later" and it asks again at the next start.
- You can play the keyboard while it fetches. Sounds not fetched yet play with a sine wave instead. Each sound plays for real as soon as it is here (the grand piano comes first).
- If the internet connection breaks partway, what was fetched is kept. Next time it goes on from there.
- It is kept in `~/.local/share/meloet/sounds/` (or `meloet/sounds/` under `XDG_DATA_HOME` if you set it).
- meloet looks for a newer pack at most once a day. It reads only one catalog file and sends nothing such as your name.
- meloet connects to the internet only to fetch the sound pack and to see whether a new version is out (see "New version notice" below).

## Plugins

meloet reads VST3 and LV2 plugins. After the first start it scans these places and lists what it finds on the shelf under "Added later".

| Format | Places |
|---|---|
| VST3 | `~/.vst3`, `/usr/lib/vst3`, `/usr/local/lib/vst3` |
| LV2 | `~/.lv2`, `/usr/lib/lv2`, `/usr/local/lib/lv2` (and the places in `LV2_PATH`) |

For plugins in other places, add the place with "Add a place…" in "Sound settings ▸ Advanced settings".

Plugins run in a process apart from meloet (`meloet-plughost`). If a plugin stops, meloet and your song are safe.

## What you can do (added after 0.3.0)

If you forget a key, use Help ▸ Key list. Type "Loop" or "Ctrl" in the box at the top to filter it. Esc closes it. "Help ▸ Open the user guide" opens this guide in your browser.

### Playing and recording

- Keep last (Shift R): even without pressing record, what you just played can become a clip. What you played along with the song goes where you heard it; what you played while stopped goes from the start of the bar of the play line.
- Play with PC keys (Ctrl K): even without a MIDI keyboard, you can play and record with the letter keys of the PC. A S D F G H J K L ; are the white keys, W E T Y U O P the black keys, and Z / X move the octave down and up. While it is on, one-letter shortcuts are off. Esc stops it. There is no sound while an input method (such as Japanese input) is on.
- Arpeggio: turn on "Arpeggio" for the track and hold a chord, and it becomes a phrase in time with the song's tempo (up / down / up and down / random, 1/4 to 1/32 and triplets, 1 to 3 octaves, keep going after you let go). Holding one key repeats it. What sounds goes into the recording as notes. It does not change the notes already in the song.
- Metronome settings (the ▾ of the metronome): volume, three sounds (beep, tick, cowbell), only when recording, and tapping the tempo (Tap).

### Piano roll

- With scale lock on, the rows of the notes of the scale look lighter and you can write only those notes (nine kinds: major, minor, harmonic minor, melodic minor, major pentatonic, minor pentatonic, blues, Dorian, Mixolydian).
- Chord progressions (Royal road, Canon, Komuro, Circle, Pop, Sad): listen to one and place it, one chord per bar (with scale lock on).
- Change the velocity of the selected notes by percent, ramp it, or spread it. "Stretch to next" makes each note last until the next one.
- See the notes of other tracks faintly ("Show other tracks"). Use it to write along with other parts. Choose the tracks to show with the ▾.
- While playing, the notes the play line passes light up in the track's color (switch it with "View ▸ Light up sounding notes"; on at first).

### Arrange

- Repeat clips: pull the handle at the right end of a clip's name to the right, or type the number of times with "Repeat…" (Ctrl R). The copies follow right after it, and one undo takes them back. If several clips are selected, they repeat as one block.
- Move whole sections: right-click a marker to duplicate its section (up to the next marker) right after it, delete it and close the gap, or insert empty bars before it. Clips, automation, tempo, time signature and markers move together, and one undo takes it back. It can be used when both ends of the section are at the start of a bar.
- Duplicate a track (right-click the track head ▸ "Duplicate track"): copies it right below, with its instrument, effects, mixer settings, clips and automation. Plugins start from the state they are in now.
- Turn a keys track or clips into audio clips (from the right-click menu). The sound through the instrument and effects is put on a new vocal (audio) track right below, and the original track is muted. It can be undone.
- Drum patterns: "Patterns" on the shelf has 12 one-bar drum patterns (8 beat, 16 beat, four on the floor, shuffle, fills…). Listen with ▶ and put one at the bar of the play line with Enter. Press Enter again to put the next one in the next bar.

### Vocal clips

- Comping: from several recorded takes of a vocal, join only the best parts into one. Right-click a vocal clip and choose "Comping", and the takes line up under the track. Drag sideways over the take you want, and that take plays in that range. The joins are smoothed automatically. "Undo comping" goes back to one take at any time.
- Pull to scale: pulls the pitch of a vocal clip toward the notes of the scale (strength 0 to 100 %). For a single voice, or an instrument that plays one note at a time. Set the strength in "Pull to scale" of "Selected clip" in the right panel. It pulls toward the song's scale at the moment you raise the strength from 0. To change it later, press "Use the current scale".
- Reverse an audio clip, or normalize it (bring its loudest point to −1.0 dB). From the clip's right-click menu. Both can be undone.

### Sounds and the mixer

- The sounds that come with meloet have four knobs in the params panel: "Brightness", "Attack", "Release", "Vibrato" (drum kits have only "Brightness" and "Release"). Like other parameters, they can be moved by automation.
- There are now nine built-in effects: gate (removes noise between sounds), de-esser (softens harsh S sounds of a vocal) and saturation (adds warmth or roughness) were added.
- Presets of the built-in effects: choose settings that come with meloet (such as "Even vocal" of the compressor). You can also name the current settings and keep them on this PC.
- Automation of the master's volume and a fade-out: in the "Edit" menu, "Fade out the end of the song…", type the length (in bars), and the master's volume goes down toward the end of the song. The line can be edited like other automation, and it goes into exported files as it is.
- "Finishing meters" right of the master in the mixer: the loudness now (the last 3 seconds) and of the whole song (LUFS), the true peak (dBTP), 24 bars of the master's sound, and "Listen in mono", which mixes left and right (only for listening; the export does not change).
- A notice when the sound is too loud (the master's limiter keeps holding it down, or the master went over 0 dB), with a button that lowers all tracks at once.
- A picture that moves with the sound: left of "Master" in the top band, the master's sound in 24 bars from low to high (turn it off with "View ▸ Show the sound moving").

### Export

- You can also export to an mp3 file. In "Details", choose the "Bitrate" from 128 / 192 / 256 / 320 kbps (192 at first). The sample rate is 44100 Hz or 48000 Hz.
- Choose "Match for release" in the export to bring the loudness of the whole song to −14 LUFS (with the true peak up to −1 dBTP). The check before export shows the loudness of the whole song (LUFS) and the true peak (dBTP).
- In "Details" of the export, you can write the song info (title, artist, picture) into the file (the picture with flac and mp3; png or jpg, up to 5 MB). The artist is remembered.
- When the export is done, "Exported" shows the waveform and the loudness (LUFS and true peak). "Play in the PC's player" plays it right there.

### Files

- File ▸ Recent songs: choose from the last 10 songs to open (their places show faintly). "Clear the list" empties it.

### Language

- View ▸ ことば / Language switches the screen between Japanese (日本語) and English. At first it follows the PC's language. Switching loads the screen again (the song and the sound stay). New songs and tracks get their names in the language they are made in (names already there do not change). In English, "Open the user guide" opens this guide.

### New version notice

- When a new version is out, the status line tells you while stopped (at most once a day; only meloet's version is sent; turn it off with "Check for updates" in the sound settings). Installing it is done by hand (download the deb and install it).

### New keys

| Key | What it does |
|---|---|
| Shift R | Keep last |
| Ctrl K | Play with PC keys (on / off) |
| Ctrl R | Repeat clips (the box for the number of times) |

## Link to AI

You can let AI apps (such as Claude Code or Claude Desktop, which speak the MCP protocol) use the running meloet. What the AI can do:

- Read the song and give advice.
- When asked, write in the song (notes, mixer, effects, sounds, automation, tracks and clips, save, open, export). Each thing it writes can be undone one step at a time. Several changes can also be made into one step.
- Read a MIDI file (.mid) into the song. Write the song's notes out to a MIDI file.
- Choose and save plugin presets. Read the whole settings of a plugin and put them back. With Surge XT it can also change what is inside a patch (the modulation routing, such as vibrato by an LFO).
- Find out which key plays which drum on a drum track.
- Name a combination of mutes, keep it in the song, and call it up later.
- Find song files on this PC (recent songs, the songs folder).
- Check exported sound and tell you numbers such as the loudness. It also checks which ranges from low to high are sounding, and whether two tracks clash at the same pitch. To learn a sound, it can also export a single note.

### Turning it on

Turn on "Sound settings ▸ Advanced settings ▸ Link to AI". It is off at first.

- Only while it is on, meloet opens a door inside this PC for AI apps to connect to. Only apps running as the same user can connect. Other people and the internet cannot.
- On the right, the state of the door shows: "Closed" (off), "Waiting" (on, no AI connected yet), "Connected".
- Turning it off closes the door, and a connected AI is cut off too.
- meloet remembers on / off (it is not in the song).
- Audio files exported for the AI go into a temporary folder only the same user can read (`/tmp/meloet-ai-<user number>`), and are deleted after an hour, when "Link to AI" is turned off, or when meloet ends. The AI (Claude) does not hear the sound with ears; the AI app reads the file with its own program.
- While there are changes not saved, the AI does not open a song or make a new song.
- AI apps may send what they read of the song (notes, tempo and so on) to the servers of the company that makes the app. For what is sent and how it is used, see the rules of that app.

### Registering meloet in the AI app

The AI app starts `/usr/bin/meloet-mcp`, which connects to meloet's door. You may start the AI app first. When you turn on "Link to AI" in meloet, it connects by itself the next time you ask the AI. While recording or exporting, the AI cannot change the song.

In Claude Code, type this once in a terminal:

```bash
claude mcp add --transport stdio --scope user meloet -- /usr/bin/meloet-mcp
```

In Claude Desktop (the Linux version is a beta), write this in its settings file `~/.config/Claude/claude_desktop_config.json`. If something is written there already, add the `meloet` line inside `mcpServers`. Then start Claude Desktop again.

```json
{"mcpServers": {"meloet": {"command": "/usr/bin/meloet-mcp"}}}
```

Where the settings file is and how to write it may change with the version of the AI app.

### Taking back what the AI wrote

What the AI writes in the song shows in the undo history one step at a time, like your own edits. If you do not like it, go back with "Undo" (Ctrl+Z) or the "Undo history" band (H). Changes the AI made together are one step, so one undo takes them back.

While the AI makes changes together (usually only a moment), your own actions wait a little before they happen. The keyboard sounds as usual.

## When something goes wrong

### No sound

- If the status line says "Out: not yet", no output has been chosen. Choose one in "Sound settings" (Ctrl+,).
- If nothing is plugged into the chosen output, the status line says so. Plug in headphones or speakers, or choose another output.
- Check that PipeWire is running. Type `systemctl --user status pipewire wireplumber` in a terminal; if both say `active (running)`, it is running. If not, install them with `sudo apt install pipewire wireplumber` and log in again.
- Also check the mutes of the tracks and the master, and where the faders are.

### The keyboard does not show

- Try plugging the USB cable in again.
- Type `aconnect -l` in a terminal and see whether the keyboard's name shows (`aconnect` is in `alsa-utils`). If it does not show there either, the PC has not found the keyboard. Try another USB port or another cable.
- Some keyboards have a power switch, or a switch for the USB MIDI mode. See the keyboard's manual.

### The sound pack cannot be fetched

- If it says "not connected to the internet", press "Again" once connected. What was fetched is kept.
- If it says "the disk is full", make the space it asks for on the disk where `~/.local/share` is.
- If it says it "came broken", wait a while and try again.

### Seeing what happens (the log)

meloet writes what happens when something goes wrong to the terminal. Start it by typing `meloet` in a terminal, not from the list of apps, and it shows there. When you tell us about a problem, please copy those lines.

## Removing meloet

```bash
sudo apt remove meloet
```

meloet itself and `meloet-plughost` are removed. These stay in your home folder. If you do not need them, delete them by hand.

| Place | What it is |
|---|---|
| `~/.config/meloet/` | Settings (output, scale, plugin places you added and so on), your own effect presets, the day it last looked for a new version |
| `~/.local/share/meloet/sounds/` | The sound pack (about 240 MB) |
| `~/.local/share/meloet/sf2/` | Copies of the SF2 files you added |
| `~/.local/state/meloet/` | The recovery file (changes not saved) |
| `~/.cache/meloet/` | Plugin scan records, waveforms and other things that can be made again |

The songs you made (at first under your music folder, such as `~/Music/`) are your files, so they stay when meloet is removed.

```bash
rm -rf ~/.config/meloet ~/.local/share/meloet ~/.local/state/meloet ~/.cache/meloet
```

(If you set `XDG_CONFIG_HOME` and the like, it is `meloet/` under those.)
