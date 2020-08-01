#!/usr/bin/env python3

import os
import shutil
import sys
from pathlib import Path
from typing import List, Optional, Sequence

import click
import gpxpy
import maya
import sh

from dataclasses import dataclass, field

# not an exaustive list
# see https://sno.phy.queensu.ca/~phil/exiftool/TagNames/Canon.html
# https://fossies.org/linux/Image-ExifTool/lib/Image/ExifTool/Canon.pm
TZ_CITIES = {
    "Austin": (28, "-06:00"),
    "Buenos Aires": (25, "-04:00"),
    "Dublin": (20, "+00:00"),
    "London": (20, "+00:00"),
    "New York": (27, "-05:00"),
    "Rome": (19, "+01:00"),
    "San Francisco": (30, "-08:00"),
    "Santiago": (25, "-04:00"),
    "Singapore": (7, "+08:00"),
    "Kiev": (17, "+02:00"),
    "US/Central": (28, "-06:00"),
    "US/Eastern": (27, "-05:00"),
    "US/Pacific": (30, "-08:00"),
}


@dataclass
class Config:
    suffixes: List[str] = field(default_factory=lambda: ["mp4", "jpg"])
    images_dir: Optional[Path] = None
    timerange: int = 10
    timezone: str = "Europe/Dublin"
    timezone_dst: bool = False
    timezone_id: int = 0


def run(command_str, *args, **kwargs):
    command = getattr(sh, command_str)
    baked = command.bake(*args, **kwargs)
    click.echo(f"Running: {baked}")

    return baked(_fg=True)


def gpx_name(_ctx: click.Context, gps_file: Path) -> Path:
    if gps_file.suffix != ".gpx":
        return gps_file.parent / f"{gps_file.stem}.gpx"

    if gps_file.name == "all_activities.gpx":
        return gps_file

    with open(gps_file) as file_:
        gpx = gpxpy.parse(file_)

    track_name = gpx.tracks[0].name
    track_time = maya.parse(gpx.time).datetime().strftime("%Y-%m-%d.%H:%M:%S")
    name = f"{track_time}_{track_name}"

    return gps_file.parent / f"{name}.gpx"


def ensure_gpx(ctx: click.Context, gps_file: Path) -> Path:
    dest = gpx_name(ctx, gps_file)

    if gps_file.suffix == ".gpx":
        if gps_file != dest:
            click.echo(f"{gps_file} -> {dest}")
            shutil.move(gps_file, dest)

    elif gps_file.suffix == ".tcx":
        run("gpsbabel", "-i", "gtrnctr", "-f", gps_file, "-o", "gpx", "-F", dest)

    else:
        ctx.fail(f"Unknown format {gps_file.suffix}")

    return dest


def merge_gpx(ctx: click.Context, gpx_files: List[Path]) -> Path:
    dest = ctx.obj.images_dir / "all_activities.gpx"
    try:
        os.remove(str(dest.resolve()))
    except OSError:
        pass

    options = ["-i", "gpx"]

    for path in gpx_files:
        if path.name == "all_activities.gpx":
            continue
        options.extend(["-f", str(path.resolve())])
    options.extend(["-o", "gpx", "-F", dest])
    run("gpsbabel", *options)

    return dest


def geotag_images(ctx: click.Context, gpx: Path) -> None:
    run(
        "gpicsync",
        "-g",
        gpx,
        "-z",
        "UTC",
        "-d",
        ctx.obj.images_dir,
        "--time-range",
        ctx.obj.timerange,
    )


def clean(ctx: click.Context) -> None:
    for x in ctx.obj.images_dir.glob("*_original"):
        os.unlink(x)


@click.group()
@click.option(
    "--images-dir",
    "-d",
    type=click.Path(writable=True, file_okay=False, exists=True, resolve_path=True),
)
@click.option(
    "--timezone", "-z", default="Europe/Dublin", type=click.Choice(TZ_CITIES.keys())
)
@click.option("--dst/--no-dst", default=False, help="Set if timezone in DST")
@click.option(
    "--timerange",
    type=int,
    default=10,
    help="Range in seconds to search a GPS position for pictures",
)
@click.option(
    "--suffix",
    "--ext",
    "-e",
    default="jpg,mp4",
    help="Image files extension (comma separated)",
)
@click.pass_context
def cli(
    ctx: click.Context,
    suffix: str,
    timerange: int,
    timezone: str,
    dst: bool,
    images_dir: Optional[str],
) -> None:
    images_dir = images_dir or "."

    tz_id, tz_info = TZ_CITIES[timezone]

    config = Config(
        suffixes=[s.lower().strip(".") for s in suffix.split(",")],
        images_dir=Path(images_dir),
        timerange=timerange,
        timezone=tz_info,
        timezone_dst=dst,
        timezone_id=tz_id,
    )
    ctx.obj = config
    clean(ctx)


@cli.command()
@click.pass_context
def rename(ctx: click.Context) -> None:
    """ Rename images using their date and time """
    dir_ = str(ctx.obj.images_dir.resolve())

    for suffix in ctx.obj.suffixes:
        up_ext = suffix.upper()

        if ctx.obj.images_dir.glob(f"*.{up_ext}"):
            if sys.platform.startswith("darwin"):
                run(
                    "rename",
                    f"s/.{up_ext}$/.{suffix}_/",
                    f"{ctx.obj.images_dir}/*.#{up_ext}",
                )
                run(
                    "rename",
                    f"s/.{suffix}_$/.{suffix}/",
                    f"{ctx.obj.images_dir}/*.#{suffix}_",
                )
            else:
                run(
                    "rename",
                    f"s/.{up_ext}$/.{suffix}/",
                    f"*.{up_ext}",
                    _cwd=ctx.obj.images_dir,
                )

    run("find", dir_, "-type", "f", "-exec", "chmod", "0644", "{}", "+")

    run(
        "exiftool",
        "-FileName<DateTimeOriginal",
        "-d",
        "%Y-%m-%d %H.%M.%S%%-c.%%e",
        "-overwrite_original",
        dir_,
    )
    clean(ctx)


@cli.command()
@click.pass_context
def set_time(ctx: click.Context) -> None:
    """ set time and timezone on pictures """
    dst = 0 if not ctx.obj.timezone_dst else 60
    shift = ctx.obj.timezone[1:]
    direction = ctx.obj.timezone[0]
    run(
        "exiftool",
        f"-AllDates{direction}=0:0:0 {shift}:0",
        f"-TimeZone={ctx.obj.timezone}",
        f"-TimeZoneCity#={ctx.obj.timezone_id}",
        f"-OffSetTime={ctx.obj.timezone}",
        f"-OffSetTimeOriginal={ctx.obj.timezone}",
        f"-OffSetTimeDigitized={ctx.obj.timezone}",
        f"-DaylightSavings#={dst}",
        "-overwrite_original",
        str(ctx.obj.images_dir.resolve()),
    )
    clean(ctx)


@cli.command()
@click.argument("gps_files", nargs=-1, type=click.Path(exists=True, dir_okay=False))
@click.pass_context
def geotag(ctx: click.Context, gps_files: Optional[Sequence[str]]) -> None:
    """ geotag images using gpx files """

    if not gps_files:
        ctx.fail("No gps files provided")

    gps_paths: List[Path] = []

    for path in gps_files:
        gps_paths.append(ensure_gpx(ctx, Path(path)))

    if len(gps_paths) > 1:
        gpx = merge_gpx(ctx, gps_paths)
    else:
        gpx = ctx.obj.images_dir / gps_paths[0]

    geotag_images(ctx, gpx)
    clean(ctx)


@cli.command()
@click.option("--reset-tz/--no-reset-tz", default=False)
@click.argument("by", nargs=1)
@click.argument("images", nargs=-1, type=click.Path(exists=True, dir_okay=True))
@click.pass_context
def shift(
    ctx: click.Context, reset_tz: bool, by: str, images: Optional[Sequence[str]]
) -> None:
    """ shift photos - this will also clear out timezones"""
    if not by:
        ctx.fail("empty shift pattern")

    if not images:
        ctx.fail("No images provided")

    if by[0] not in ("+", "-"):
        direction = "+"
    else:
        direction = by[0]
        by = by[1:]

    args = [
        f"-AllDates{direction}=0:0:0 {by}:0",
        "-overwrite_original",
    ]

    if reset_tz:
        args.extend(
            [
                "-OffSetTime=",
                "-OffSetTimeOriginal=",
                "-OffSetTimeDigitized=",
                "-Timezone=" "-TimezoneCity=",
            ]
        )

    run(
        "exiftool", *args, *[i for i in images],
    )
    clean(ctx)


@cli.command()
@click.argument("gps_files", nargs=-1, type=click.Path(exists=True, dir_okay=False))
@click.pass_context
def all(ctx: click.Context, gps_files: Optional[Sequence[str]]) -> None:
    ctx.forward(geotag)
    ctx.invoke(set_time)
    ctx.invoke(rename)


def main():
    cli(obj=Config())
