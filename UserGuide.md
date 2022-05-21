Introduction
============

`bliss-analyser` is a command-line application to scan your music collection and
upload its database of music analysis to LMS. The `Bliss Mixer` LMS plugin can
then use this information to provide music mixes for LMS's `Don't Stop the Music`
feature.

**NOTE:** You must run this application from a terminal window (e.g. cmd.com or
PowerShell for Windows), as there is no graphical user interface.


Quick guide
===========

1. Install the `Bliss Mixer` LMS plugin.

2. Install ffmpeg if using Linux or macOS.

3. Edit the supplied `config.ini` in the current folder to set appropiate values
for `music` and `lms` - e.g.:
```
[Bliss]
music=/home/user/Music
lms=127.0.0.1
```

4. Analyse your tracks:
```
./bliss-analyser analyse
```

5. Upload analysis database to LMS:
```
./bliss-analyser upload
```

6. Set LMS to use `Bliss` in `Don't Stop the Music`

7. Play some music!



Installation
============

For Windows no extra installation steps are required, as all dependencies are
bundled within its ZIP file. However, both the Linux and macOS versions require
that `ffmpeg` be installed.


Linux
-----

Debian based systems (e.g. Ubuntu):
```
sudo apt install ffmpeg
```

RedHat based systems (e.g. Fedora):
```
sudo yum install ffmpeg
```


macOS
-----

First install `HomeBrew`

High Sierra, Sierra, El Capitan, or earlier:
```
/usr/bin/ruby -e "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
```

Otherwise:
```
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
```

Then install `ffmpeg`:
```
brew install ffmpeg@5
brew link ffmpeg@5
```



Configuration
=============

`bliss-analyser` can (optionally) read its configuration from an INI-style file.
By default `bliss-analyser` looks for a file named `config.ini` in its current
folder, however the exact name and location can be specified as a command-line
parameter. This file has the following syntax:

```
[Bliss]
music=/home/user/Music
db=bliss.db
lms=127.0.0.1
ignore=ignore.txt
```

The following items are supported:
* `music` specifies the location of your music collection - e.g. `c:\Users\user\Music`
for windows. This default to `Music` within the user's home folder. Up to 4 other
music folders may be specified via `music_1`, `music_2`, `music_3`, and `music_4`
* `db` specifies the name and location of the database file used to store the
analysis results. This will default to `bliss.db` in the current folder.
* `lms` specifies the hostname, or IP address, of your LMS server. This is used
when uploading the database file to LMS. This defaults to `127.0.0.1` If your LMS is
password protected then use `user:pass@server` - e.g. `lms=pi:abc123@127.0.0.1`
* `ignore` specifies the name and location of a file containing items to ignore
in mixes. See the `Ignore` section later on for more details.



Command-line parameters
=======================

`bliss-analyser` accepts the following optional parameters:

* `-c` / `--config` Location of the INI config file detailed above.
* `-m` / `--music` Location of your music collection,
* `-d` / `--db` Name and location of the database file.
* `-l` / `--logging` Logging level; `trace`, `debug`, `info`, `warn`, `error`.
Default is `info`.
* `-k` / `--keep-old` When analysing tracks, `bliss-analyser` will remove any
tracks specified in its database that are no-longer on the file-system. This
parameter is used to prevent this.
* `-r` / `--dry-run` If this is supplied when analysing tracks, then no actual
analysis will be performed, instead the logging will inform you how many new
tracks are to be analysed and how many old tracks are left in the database.
* `-i` / `--ignore` Name and location of the file containing items to ignore.
* `-L` / `--lms` Hostname, or IP address, of your LMS server.
* `-n` / `--numtracks` Specify maximum number of tracks to analyse.

Equivalent items specied in the INI config file (detailed above) will override
any specified on the commandline.

`bliss-analyser` requires one extra parameter, which is used to determine the
required task. This takes the following values:

* `analyse` Performs analysis of tracks.
* `upload` Uploads the database to LMS.
* `stopmixer` Asks LMS plugin to stop it instance of `bliss-mixer`
* `tags` Re-reads tags from your music collection, and updates the database for
any changes.
* `ignore` Reads the `ignore` file and updates the database to flag tracks as
to be ignored for mixes.



Analysing tracks
================

Before you can create any mixes, your tracks need to be analysed. Assuming
`config.ini` is in the current folder and contains valid entries, this is
accomplished as follows:

(Linux / macOS)
```
./bliss-analyser analyse
```

(Windows)
```
.\bliss-analyser.exe analyse
```

This will first iterate all sub-folders of your music collection to build a list
of filenames to analyse. New tracks that are not currently in the database are
then analysed, and a progress bar showing the current percentage and time used
is shown.

As a rough guide, a 2015-era i7 8-core laptop with SSD analyses around 14000
tracks/hour.


CUE files
---------

If the anlyser encounters an audio file with a matching CUE file (e.g.
`album.flac` and `album.cue` in same folder) then it will attempt to analyse the
individual tracks contained within.


Exclude folders
---------------

If you have audio books, or other audio items, within your music folder that you
do not wish to have analysed, you can prevent `bliss-analyser` from analysing
these be creating a file named `.notmusic` within the required folder. e.g.

```
/home/user/Music/Audiobooks/.notmusic
```



Uploading database
==================

Once your tracks have been analysed, you need to `upload` your database to LMS
so that its plugin can then use this information to create mixes. Assuming
`config.ini` is in the current folder and contains valid entries, this is
accomplished as follows:

(Linux / macOS)
```
./bliss-analyser upload
```

(Windows)
```
.\bliss-analyser.exe upload
```

If your LMS is running on the same machine as `bliss-analyser` and you have set
the db path to be the location within your LMS's `Cache` folder which
`bliss-mixer` will use to access `bliss.db`, then there is no need to 'upload'
the database and all you need to do is stop any running `bliss-mixer`. This can
be accomplished manually, or via the following:

(Linux / macOS)
```
./bliss-analyser stopmixer
```

(Windows)
```
.\bliss-analyser.exe stopmixer
```

*NOTE* You must already have the `Bliss Mixer` LMS plugin installed, or you will
not be able to upload the database.



Re-reading tags
===============

If you have changed the tags in some files then the analysis database will have
the old tags. To update this database with the changed tags, run `bliss-analyser`
as follows (assuming `config.ini` is in the current folder and contains valid
entries):

(Linux / macOS)
```
./bliss-analyser tags
```

(Windows)
```
.\bliss-analyser.exe tags
```

*NOTE* Tag re-reading is not implemented for CUE tracks. Therefore if you update
your CUE files you will need to remove these from the database and re-analyse,
or manually edit the meta-data in `bliss.db` using an SQLite GUI.



Ignoring tracks in mixes
========================

Its possible that you have some tracks that you never want added to mixes, but
as these are in your music collection they might be in your music queue and so
could possibly be chosen as `seed` tracks for mixes. Therefore you'd want the
analysis in the database, so that you can find mixable tracks for them, but
would not want them be chosen as mixable tracks from other seeds. This is
accomplished be setting the `Ignore` column to `1` for such tracks. To make this
easier, `bliss-analyser` can read a text file containing items to ignore and
will update the database as appropriate.

This `ignore` file is a plain text file where each line contains either:

1. The unique path to be ignored. i.e. it could contain  the complete path
(relative to your music folder) of a track, an album name (to exclude a whole
album), or an artist name (to exclude all tracks by the artist).
2. An SQL selector. If so, line must start "SQL:" followed by code that will be
run after WHERE

```
ABBA/Gold - Greatest Hits/01 Dancing Queen.mp3
AC-DC/Power Up/
The Police/
SQL:Genre='Blues'
SQL:Genre LIKE '%Dance%'
SQL:Genre='Rock'
SQL:Genre LIKE 'Rock;%'
SQL:Genre LIKE '%;Rock'
SQL:Genre LIKE '%;Rock;%'
```

This would exclude 'Dancing Queen' by ABBA, all of AC/DC's 'Power Up', all
tracks by 'The Police', all tracks with 'Genre' set to 'Blues', and all tracks
that have 'Dance' or 'Rock'  as part of their 'Genre'.

The SQL LIKE lines do sub-string matching. So '%Dance%' will match any genre
string that contains 'Dance' - e.g. 'Classical Dance'. The 4 lines with 'Rock'
show how you can explicitly look for an exact match. The 1st line means 'Rock'
is the only genre, 2nd means 'Rock' is the first genre, 3rd means 'Rock' is the
last genre, and 4th means 'Rock' is amongs other genres.

Assuming `config.ini` is in the current folder and contains valid entries, this
is accomplished as follows:

(Linux / macOS)
```
./bliss-analyser ignore
```

(Windows)
```
.\bliss-analyser.exe ignore
```



Credits
=======

The actual music analysis is performed by the `bliss-rs` library. See
https://lelele.io/bliss.html for more information on this.
