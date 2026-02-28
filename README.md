# exif-chdate
A tool to change the day, month and optionally year of image files.
The time is intentially left unchanged to allow for perserving
the chronologic order of images. This tool is useful when, for example,
needing to have a more accurate date when scanning negatives using a
digital camera.

Requires `exiftool` to be installed locally.

## Usage

`exif-chdate <day> <month> [year] path/to/images/*.RAW`
