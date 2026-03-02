# exif-film
An opinionated tool to set EXIF metadata of scanned film negatives.

Requires `exiftool` to be installed locally.

## Usage

```
exif-film <year>::<month>::<day> <film> <process> <camera> <lens> <file1> [file2 ...]

<year>:<month>:<day>    Example: 1999:01:01
<film>                  Type of film and ISO. Example: Ilford HP5+ @1600
<process>               Film process. Example: Rodinal 1+25 @1600
<camera>                Original camera
<lens>                  Original lens
<file…>                 One or more image files to modify

The date will overwrite the `DateTimeOriginal` tag, while the rest of the fields will overwrite
the `UserComment` tag separated by `;`. The `@` character is a convention and meant to be used as
a marker that the following numeric token is an ISO identifier allowing you to provide
"shot at" and "processed at" ISO values.
```
