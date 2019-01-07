# photo_process

simple scripts -- more or less an evolution from a bash script running simple commands.
Better if you just move on

## install

You'd need python 3.7+ and [pipenv](https://pipenv.readthedocs.io/en/latest/)
you also need, working and reachable in `$PATH`

* gpsbabel
* gpicsync (watch out this one is python2-only)
* exiftool

```
$ python --version
Python 3.7.1
$ pipenv install -d
Installing dependencies from Pipfile.lock (166c9c)…
  🐍   ▉▉▉▉▉▉▉▉▉▉▉▉▉▉▉▉▉▉▉▉▉▉▉▉▉▉▉▉▉▉▉▉ 34/34 — 00:00:06
```

## run

make sure the pictures are in UTC.
make sure your gpx file is also UTC

run

```
$ photo_process -z <timezone> -d /path/to/pics all /path/to/gpx/*.gpx
```

this will geotag the pictures, shift times to the new timezone and rename the files.
