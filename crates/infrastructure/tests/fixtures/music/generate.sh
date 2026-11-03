#!/bin/sh
# Regenerate the CC0 synthetic silent fixtures. Test runners do not need ffmpeg.
set -eu
cd "$(dirname "$0")"
for pair in 'mp3 libmp3lame' 'flac flac' 'm4a aac' 'ogg libvorbis' 'opus libopus' 'wav pcm_s16le'; do
  set -- $pair
  ffmpeg -hide_banner -loglevel error -y -f lavfi -i anullsrc=r=44100:cl=mono -t 0.08 -c:a "$2" -metadata title='Original title' -metadata artist='Synthetic artist' -metadata comment='Keep this comment' "silence.$1"
done
