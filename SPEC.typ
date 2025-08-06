= Project Description: A Rule-Based Personal Media Archival System

== 1. Synopsis

This document describes a command-line utility designed for the robust and automated organization of a personal media library. It empowers individuals to transform a chaotic collection of photos and videos into a structured, meaningful, and permanent archive. At its core, the system uses a powerful, declarative rule engine to manage files based on their rich metadata, allowing for sophisticated and personalized organizational schemes.

== 2. Core Philosophy: Declarative Archival Pipelines

_The fundamental principle is that every photograph and video is a record of a specific moment at a specific location._ This software is built to honor that principle. Its core philosophy is to empower the user to define an entire media management workflow that translates this innate space-time context into a logical, permanent archive.

This is achieved through a declarative pipeline model. Instead of writing complex, imperative scripts, the user specifies a series of processing *stages*. These stages can be chained together, creating a processing *pipeline* where media flows from one stage to the next, being organized, filtered, and transformed according to user-defined logic. This approach turns media management from a recurring manual chore into an automated, repeatable, and transparent process perfectly suited for creating and maintaining a personal life archive.

== 3. The Data Acquisition Pipeline

Before any organizational rules can be applied, the system must first build a comprehensive data profile for each media file. This is done via a multi-stage data acquisition pipeline.

=== Stage 1: Ingestion
- *Action*: A media file is identified as a candidate for processing.

=== Stage 2: Temporal Analysis (The "When")
- *Goal*: Determine the most accurate creation timestamp.
- *Priority Order*:
  1. *EXIF Metadata*: Use `DateTimeOriginal` from EXIF data.
  2. *Filesystem Fallback*: If EXIF is absent, use the oldest of `mtime` or `ctime`.

=== Stage 3: Spatial Analysis (The "Where")
- *Goal*: Identify geographic coordinates of capture.
- *Priority Order*:
  1. *EXIF GPS Data*
  2. *Google Maps History* (fallback using Stage 2 timestamp)

=== Stage 4: Data Augmentation & Enrichment
- *Goal*: Expand raw data into variables for rule engine.
- *Actions*:
  - *Temporal*: Derive year, month, weekday, etc.
  - *Spatial*: Reverse-geocode coordinates.
  - *Technical*: Extract dimensions, duration, MIME type, etc.

== 4. Data Context & Template Variables

#table(
  columns: (15%, 25%, 35%, 25%),
  [*Category*], [*Variable*], [*Description*], [*Example*],

  [time], [{time.yyyy}], [4-digit year], [2025],
  [], [{time.yy}], [2-digit year], [25],
  [], [{time.mm}], [2-digit month (01-12)], [07],
  [], [{time.dd}], [2-digit day of month], [18],
  [], [{time.month_name}], [Full name of the month], [July],
  [], [{time.month_short}], [3-letter month abbreviation], [Jul],
  [], [{time.day_name}], [Day of the week], [Friday],
  [], [{time.day_short}], [Abbrev. day], [Fri],
  [], [{time.HH}], [Hour (00-23)], [21],
  [], [{time.MM}], [Minutes (00-59)], [30],
  [], [{time.SS}], [Seconds (00-59)], [05],

  [space], [{space.country}], [Country name], [Spain],
  [], [{space.country_code}], [2-letter country code], [ES],
  [], [{space.state}], [State/Province/Region], [Community of Madrid],
  [], [{space.city}], [City or town], [Madrid],
  [], [{space.county}], [County or district], [Madrid],
  [], [{space.road}], [Street name], [Calle de Atocha],

  [source],
  [{source.path}],
  [Original file path],
  [/home/nil/import/IMG_1234.JPG],

  [], [{source.name}], [Filename without extension], [IMG_1234],
  [], [{source.extension}], [Lowercase extension], [jpg],
  [], [{source.original}], [Full original filename], [IMG_1234.JPG],

  [media], [{media.type}], [Media type], [image],
  [], [{media.mimetype}], [MIME type], [image/jpeg],
  [], [{media.size}], [File size (bytes)], [4194304],
  [], [{media.width}], [Width in pixels], [4000],
  [], [{media.height}], [Height in pixels], [3000],
  [], [{media.resolution}], [`WxH` resolution], [4000x3000],
  [], [{media.duration}], [Duration (sec)], [183.5],

  [special], [{special.md5_short}], [First 8 of MD5], [a1b2c3d4],
  [], [{special.count}], [Filename collision suffix], [\_1],
)

== 5. The Ruleset Engine

This system's core logic is defined with *rulesets*, which are pipeline stages. Each ruleset has:

1. *name*: A unique identifier (e.g., `"Archive"`)
2. *input*: Defines the source of media files to process:
  - `cmdline`: Accepts CLI arguments.
  - `path: /path/to/dir`: Scans files at path.
  - `watch: /path/to/dir`: Monitors a dir daemon-style.
  - `ruleset: <name>`: Takes output from another ruleset.
3. *rules*: List of rule objects. For each file, rules are tested in order. The first matching rule executes and ends evaluation.

== 6. Rule and Action Definitions

A rule is:
- *condition*: Optional boolean test (e.g., `media.type == "video"`). Use `default` to always match.
- *template*: Defines output path using variables.
- *action*: Either built-in or custom.

=== Built-in Actions
- `move`, `copy`, `symlink`, `hardlink`

=== Custom Actions
Commands that use template variables. Custom actions may access `{target.path}` for resolved output.

== 7. Example Configuration

```yaml
# CUSTOM ACTIONS
actions:
  create-low-res-jpeg:
    command: ["magick", "{source.path}", "-resize", "1920x1080>", "-strip", "-quality", "85", "{target.path}"]

# RULESET PIPELINE
rulesets:
  # MASTER ARCHIVAL STAGE
  Master-Archive:
    input: cmdline
    rules:
      - condition: 'media.type == "video"'
        action: move
        template: "/mnt/media/Videos/{time.yyyy}/{time.yyyy}-{time.mm}-{time.dd}/{source.original}"

      - condition: 'media.type == "image" && space.city == "Madrid"'
        action: move
        template: "/mnt/media/Photos/Home/{time.yyyy}/{time.mm}/{source.original}"

      - condition: 'media.type == "image"'
        action: move
        template: "/mnt/media/Photos/Travel/{space.country}/{space.city}/{time.yyyy}-{time.mm}/{source.original}"

  # WEB GALLERY GENERATOR
  Web-Gallery-Generator:
    input: "ruleset:Master-Archive"
    rules:
      - condition: 'media.type == "image"'
        action: create-low-res-jpeg
        template: "/var/www/gallery/img/{time.yyyy}/{source.name}.jpg"

  # SCANNER DAEMON
  Scanner-Daemon:
    input: "watch:/home/nil/scans/inbox"
    rules:
      - action: move
        template: "/mnt/media/Documents/Scans/{time.yyyy}/{time.yyyy}-{time.mm}-{time.dd}_{source.name}.{source.extension}"
```
