# musicdb

A feature-rich music player consisting of a server and a client.

## Library

- search for artists, albums, songs
- apply filters to your search
- select multiple songs, albums, artists
- drag songs, albums, artists or your selection to add them to the queue

## Queue

- recursive structure, organized by how songs were added
  + adding an album puts the songs in a folder
  + if you add an album, you can drag the entire folder rather than individual songs
  + adding an artist adds a folder containing a folder for each album
- shuffle
  + works like a folder, but plays its contents in a random order
  + reshuffles as you add new songs
  + only shuffles elements that you didn't listen to yet, so you won't hear a song twice
  + can shuffle any element, so you could, for example, listen to random albums, but the songs in each album stay in the correct order
- repeat
  + can repeat its contents forever or n times

https://github.com/Dummi26/musicdb/assets/67615357/888a2217-6966-490f-a49f-5085ddcf3461

## Server

The server caches the next song before it is played,
so you get gapless playback even when loading songs from a very slow disk or network-attached storage (NAS).

It can be accessed using the client (TCP), or a website it can optionally host.
It should also be very easy to switch from TCP to any other protocol, since most of the code in this project just requires the `Read + Write` traits, not specifically a TCP connection.

## Clients

Multiple clients can connect to a server at the same time.
All connected clients will be synchronized, so if you do something on one device, all other connected devices will show that change.

The client can show a user interface (`gui`) or even connect to the server and mirror its playback (`syncplayer-*`).

Using the `syncplayer` functionality, you can play the same music on multiple devices, in multiple different locations.

https://github.com/Dummi26/musicdb/assets/67615357/afb0c9fa-3cf0-414a-a59f-7e462837b989

# Setup

## building

Run `cargo build --release` in `musicdb-filldb`, `musicdb-server`, and `musicdb-client`.

The executable files will be `musicdb-*/target/release/musicdb-*`.

### building the server

You may need to install `libasound2-dev` or similar packages,
as the default audio backend likely requires it.
If this is the case, you will likely see `error: ld returned 1 exit status`.

Multiple backends are available for playing audio,
but using `default-playback` should be fine most of the time.
If compilation fails or you experience an audio related issue,
try compiling with a different backend and see if the issue persists:

```sh
# in musicdb-server
cargo build --release --no-default-features --features website,playback-via-$backend
```

#### backends

The `rodio` and `playback-rs` backends are named after the libraries they use.
These pull in dependencies and may try to link to system libraries.

The `mpv` backend executes the `mpv` media player to play audio.
If you can't compile other backends, use this.
This backend will spawn up to two `mpv` processes at a time
and control them using unix sockets in `/tmp/`.

The `sleep` backend doesn't play any audio. Instead,
it waits for the duration of the song and then moves
on to the next one. If you want to connect
(audio-playing) servers or (web) clients
to one server where the audio files are stored but which
doesn't play any audio itself, you can use this backend.
If the duration of a song is unknown, the song will be played
for a duration of zero seconds, meaning it will just get skipped
(although connected servers or clients may still load the song
and may even succeed in playing it).

#### other features

The `website` feature is required for `--web <addr>` to work.
It allows the server to handle http requests so that it
can be controlled without using `musicdb-client`.
Turning this off improves compile times and stuff,
but leaving the feature on is quite useful
(e.g. to pause/resume playback from a phone).

### building the client

The client can play audio in sync with the server.
If you don't need this, you don't need to compile it:

```sh
# in musicdb-client
cargo build --release --no-default-features --features gui
```

Instead of disabling playback, you can also choose
from the backends available to the server.
Read "compiling the server" for more information.

In the future, it may make sense to disable the `gui` feature too
(e.g. the client may act as a cli tool for scripts), but not yet.

## setup.sh

Review, then run the `setup.sh` script:

```sh
./setup.sh ~/my_dbdir ~/music
```

Where `~/music` is the directory containing your music (mp3 files).

Confirm that all paths are correct, then press Enter when prompted.

You will probably have to add a valid font path to the client's gui config, for example

```toml
font = '/usr/share/fonts/...'

...
```

## manually

The `setup.sh` script will start a server and client.
After closing the client, the server remains active.
You have to `pkill musicdb-server` if you want to stop it.

To open the player again:

```sh
musicdb-client 0.0.0.0:26002 gui
```

To start the server without `setup.sh`:

```sh
musicdb-server --tcp 0.0.0.0:26002 --play-audio local ~/my_dbdir ~/music
```

A simple script can start the server and then the client:

```sh
# if the server is already running, this command will fail since 0.0.0.0:26002 is already in use,
# and you will never end up with 2+ servers running at the same time
musicdb-server --tcp 0.0.0.0:26002 --play-audio local ~/my_dbdir ~/music &
# wait for the server to load (on most systems, this should never take more than 0.1 seconds, but just in case...)
sleep 1
# now start the client
musicdb-client 0.0.0.0:26002 gui
```

You could use this script from a `.desktop` file to get a menu entry which simply opens the player.
