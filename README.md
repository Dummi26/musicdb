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
