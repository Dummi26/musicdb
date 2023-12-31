# musicdb

custom music player which can be controlled from other WiFi devices (phone/pc)

should perform pretty well (it runs well on my Pine A64 with 10k+ songs)

https://github.com/Dummi26/musicdb/assets/67615357/8ba85f00-27a5-4e41-8688-4816b8aaaff4

## why???

#### server/client

allows you to play music on any device you want while controlling playback from anywhere.
you can either run the client and server on the same machine or have them be in the same network
so that they can connect using TCP.

if one client makes a change, all other clients will be notified of it and update almost instantly.

it is also possible for a fake "client" to mirror the main server's playback, so you could sync up your entire house if you wanted to.

#### complicated queue

- allows more customization of playback (loops, custom shuffles, etc.)
- is more organized (adding an album doesn't add 10-20 songs, it creates a folder so you can (re)move the entire album in/from the queue)

#### caching of songs

for (almost) gapless playback, even when the data is stored on a NAS or cloud

#### custom database file

when storing data on a cloud, it would take forever to load all songs and scan them for metadata.
you would also run into issues with different file formats and where to store the cover images.
a custom database speeds up server startup and allows for more features.

## usage

### build

build `musicdb-server` and `musicdb-client` (and `musicdb-filldb`) using cargo.

Note: the client has a config file in ~/.config/musicdb-client/, which includes the path to a font. You need to set this manually or the client won't start.
The file and directory will be created when you first run the client in gui mode.

## setup

### prep

You need some directory where your music is located (mp3 files).
I will assume this is `/music` for simplicity.

You will also need a file that will hold your database.
I will assume this is `dbfile`.

Note: Instead of adding the executables (`musicdb-client` and `musicdb-server`) to your `$PATH`, you can run `cargo run --release -- ` followed by the arguments.
Since this is using cargo, you need to be in the source directory for whatever you want to run.

### database

`musicdb-filldb` will read all files in the /music directory and all of its subdirectories, read their metadata and try to figure out as much about these songs as possible. It will then generate a `dbfile` which `musicdb-server` can read.
You can make changes to the database later, but this should be the easiest way to get started:

```sh
musicdb-filldb /music
```

### starting the server

Copy the `musicdb-server/assets` directory to `./assets`, then run:

```sh
musicdb-server dbfile /music --tcp 127.0.0.1:26314 --web 127.0.0.1:8080
```

Note: If you don't care about the HTML site, you can leave out the `--web 127.0.0.1:8080`.
You also won't need the `assets` then.

And that's it - the rest should just work.

You can now open 127.0.0.1:8080 in a browser or use `musicdb-client`:

```sh
musicdb-client 127.0.0.1:26314 gui
```

### syncplayer

`musicdb-client` has a syncplayer mode, where it will play back songs in sync with the server.
It's usually easier to use syncplayer-network, which will get the song files from the server,
but syncplayer-local may be more stable and responsive, because it assumes you have a local copy of the server's music files (`/music`) somewhere, for example at `~/music`:

```sh
musicdb-client 127.0.0.1:26314 syncplayer-network
```

```sh
musicdb-client 127.0.0.1:26314 syncplayer-local ~/music
```
# TODOs

If the server can't play a song, it seems to get stuck.
It should instead send a notification to all clients about the error and skip to the next song.
The clients should then display this notification to the user.
TODO: Figure out what to do when no user acknowledges the notification. Send it again later or just pretend nothing happened?
