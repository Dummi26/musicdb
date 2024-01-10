#!/usr/bin/sh
DBDIR=$1
MUSICDIR=$2

if [ ! -z "$VISUAL" ]; then
  EDITOR="$VISUAL"
fi

echo "usage: ./setup.sh dbdir music-dir"

if [ ! -z "$DBDIR" ]; then
  echo "dbdir: $DBDIR"
  if [ ! -z "$MUSICDIR" ]; then
    echo "music-dir: $MUSICDIR"

    # set commands for filldb/server/client
    if [ ! -f "$CMD_FILLDB" ]; then
      CMD_FILLDB="musicdb-filldb/target/release/musicdb-filldb"
      if [ ! -f "$CMD_FILLDB" ]; then
        CMD_FILLDB="bin/musicdb-filldb"
        if [ ! -f "$CMD_FILLDB" ]; then
          echo "No command for filldb found, set CMD_FILLDB env var to the executable file or run \`cargo build --release\` in \`./musicdb-filldb\`."
          return
        fi
      fi
    fi
    if [ ! -f "$CMD_SERVER" ]; then
      CMD_SERVER="musicdb-server/target/release/musicdb-server"
      if [ ! -f "$CMD_SERVER" ]; then
        CMD_SERVER="bin/musicdb-server"
        if [ ! -f "$CMD_SERVER" ]; then
          echo "No command for server found, set CMD_SERVER env var to the executable file or run \`cargo build --release\` in \`./musicdb-server\`."
          return
        fi
      fi
    fi
    if [ ! -f "$CMD_CLIENT" ]; then
      CMD_CLIENT="musicdb-client/target/release/musicdb-client"
      if [ ! -f "$CMD_CLIENT" ]; then
        CMD_CLIENT="bin/musicdb-client"
        if [ ! -f "$CMD_CLIENT" ]; then
          echo "No command for client found, set CMD_CLIENT env var to the executable file or run \`cargo build --release\` in \`./musicdb-client\`."
          return
        fi
      fi
    fi
    echo "CMD_FILLDB: $CMD_FILLDB (set CMD_FILLDB env var to override)"
    echo "CMD_SERVER: $CMD_SERVER (set CMD_SERVER env var to override)"
    echo "CMD_CLIENT: $CMD_CLIENT (set CMD_CLIENT env var to override)"
    echo "EDITOR: $EDITOR (set EDITOR or VISUAL env var to override)"

    # create DBDIR
    mkdir "$DBDIR"
    
    # create DBDIR/dbfile
    echo
    "$CMD_FILLDB" "$MUSICDIR" || return
    mv "dbfile" "$DBDIR" || return
    
    # start server
    "$CMD_SERVER" "$DBDIR" "$MUSICDIR" --tcp 0.0.0.0:26002 &
    sleep 1

    # start client
    # if starting fails once, prompt user to provide a font, then try again
    "$CMD_CLIENT" 0.0.0.0:26002 gui || ("$EDITOR" "~/.config/musicdb-client/config_gui.toml"; "$CMD_CLIENT" 0.0.0.0:26002 gui) || return

  fi
fi
