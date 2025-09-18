# photo_process

simple scripts -- more or less an evolution from a bash script running simple commands.
Better if you just move on

## install

You will need to have
[uv](https://github.com/astral-sh/uv?tab=readme-ov-file#installation) installed

you also need, working and reachable in `$PATH`

* gpsbabel
* gpicsync
* exiftool

```
$ uv venv
$ uv sync
$ source .venv/bin/activate
```

## run

make sure the pictures are in UTC.
make sure your gpx file is also UTC

run

```
$ photo_process -z <timezone> -d /path/to/pics all /path/to/gpx/*.gpx
```

this will geotag the pictures, shift times to the new timezone and rename the files.
