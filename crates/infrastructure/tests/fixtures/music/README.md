# Synthetic audio fixtures

These are 0.08-second silent mono files synthesized from `anullsrc` with FFmpeg.
No user media, recording, artwork or copyrighted performance is included.
The fixture bytes and generator are dedicated to the public domain under CC0-1.0.
Run `sh generate.sh` here with FFmpeg (libmp3lame/libvorbis/libopus enabled) to regenerate.
Tests read the checked-in bytes and do not invoke FFmpeg or scan any real library.

Formats: MP3, FLAC, AAC in M4A, Vorbis in OGG, Opus in OGG, PCM in WAV.
Fixture title/artist/comment allow real Lofty preservation/read/write assertions.
