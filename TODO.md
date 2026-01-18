# tests

## gpx merging
add a test for merge gpx. Get two sample tracks, merge them with gpsbabel and
save the merged gpx
    gpsbabel -i gpx -f track1.gpx -f track2.gpx ... -o gpx -F all_activities.gpx

then add a test to see that the files are equal

## process
Add some processed files we know they are correct (shifted, geotagged). One from
camera one from every phones we have.
Have process run on it and have make sure the files match.
