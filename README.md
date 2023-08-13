# musicdb

custom music player running on my personal SBC which can be controlled from other WiFi devices (phone/pc)

should perform pretty well (it runs well on my Pine A64 with 10k+ songs)

## why???

#### server/client

allows you to play music on any device you want while controlling playback from anywhere.
you can either run the client and server on the same machine or connect via tcp.

if one client makes a change, all other clients will be notified of it and update almost instantly.

it is also possible for a fake "client" to mirror the main server's playback, so you could sync up your entire house if you wanted to.

#### complicated queue

- allows more customization of playback (loops, custom shuffles, etc.)
- is more organized (adding an album doesn't add 10-20 songs, it creates a folder so you can (re)move the entire album in/from the queue)

#### caching of songs

for (almost) gapless playback, even when the data is stored on a NAS or cloud

#### central database

when storing data on a cloud, it would take forever to load all songs and scan them for metadata.
you would also run into issues with different file formats and where to store the cover images.
a custom database speeds up server startup and allows for more features.

## usage

### build

build `musicdb-server` and `musicdb-client` using cargo.
for the client, you may need to change the path used in `include_bytes!(...)` to one that actually points to a valid font on your system.

## setup

### server/client

#### prep

You need some directory where your music is located (mp3 files).
I will assume this is `/music` for simplicity.

The structure should be `/music/Artist/Album/Song.mp3` so we can automatically
populate the database with your songs. if it isn't, songs will need to be added manually (for now).

You will also need a file that will hold your database.
I will assume this is `dbfile`.

Note: Instead of adding the executables (`musicdb-client` and `musicdb-server`) to your `$PATH`, you can run `cargo run --release -- ` followed by the arguments.
Since this is using cargo, you need to be in the source directorie for whatever you want to run.

#### initializing the server

run the server without arguments to see the current syntax.

To initialize a new database:

```sh
musicdb-server dbfile --init /music --tcp 127.0.0.1:12345 --web 127.0.0.1:8080
```

#### adding songs

While the server is running, the client can add songs to it:

##### automatically

If your files are stored as `/music/Artist/Album/Song.mp3`,
the client can go through these files and add them all to the database.
For this to work, the client should be running on the same machine as the server
(the contents of `/music` must match).

```sh
musicdb-client filldb 127.0.0.1:12345
```

You can open `127.0.0.1:8080` in a browser to see the songs being added in real-time.

##### manually

You can use the client's cli mode to manually add songs (this is annoying. don't.)

```sh
musicdb-client cli 127.0.0.1:12345
```

##### saving

```sh
musicdb-client cli 127.0.0.1:12345
```

Now, start the client's cli mode and type 'save'. The server will now create (or overwrite) `dbfile`.

And that's it - you can now use the player. For a user-friendly(er) interface, start the client in gui mode:

```sh
musicdb-client gui 127.0.0.1:12345
```

#### (re)starting the server

To load an existing dbfile, remove the `init` part from the command used earlier:

```sh
musicdb-server dbfile --tcp 127.0.0.1:12345 --web 127.0.0.1:8080
```

And that's it - the rest should just work.

### syncplayer

If `/music` is the same on two devices, one can act as the server and the other as a client
that simply mirrors the server using the client's syncplayer mode:

```sh
musicdb-client syncplayer 127.0.0.1:12345
```
