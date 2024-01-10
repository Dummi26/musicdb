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

# Setup

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

The script will start a server and client.
After closing the client, the server may still be running, so you may have to `pkill musicdb-server` if you want to stop it.

To open the player again:

```sh
musicdb-client 0.0.0.0:26002 gui
```

To start the server:

```sh
musicdb-server ~/my_dbdir ~/music --tcp 0.0.0.0:26002
```

A simple script can start the server and then the client:

```sh
# if the server is already running, this command will fail since 0.0.0.0:26002 is already in use,
# and you will never end up with 2+ servers running at the same time
musicdb-server ~/my_dbdir ~/music --tcp 0.0.0.0:26002 &
# wait for the server to load (on most systems, this should never take more than 0.1 seconds, but just in case...)
sleep 1
# now start the client
musicdb-client 0.0.0.0:26002 gui
```

You could use this script from a `.desktop` file to get a menu entry which simply opens the player.
