# 🌸 MONANA

Media Organization, Normalization, and Archival via Named Automation

> “I’m not just organizing your files — I’m preserving your life’s moments.” – Monana 💁‍♀️

---

## ✨ Overview

MONANA is a high-performance, rule-driven media archival system written in Rust. Powered by a declarative pipeline model and the Rhai scripting engine, Monana turns messy camera rolls and unsorted disks into beautifully structured media libraries — effortlessly and repeatably.

Define clear rules. Honor time and place. Archive for life.

---

## 🎯 Key Features

- 🧠 Smart Metadata Extraction  
  Parses EXIF, file system timestamps, GPS, and even cloud history to build rich temporal & spatial profiles.

- 🪄 Declarative Rule Engine  
  Write archival logic using clean rulesets and templated paths — no scripting required (unless you want it).

- ⚙️ Rhai-Powered Extensibility  
  Customize behavior with lightweight Rhai scripts — safe, embedded, and easy to learn.

- 📸 Canonical Naming + Structure  
  Organize by date, camera, country, or event. Monana enforces consistency across time.

- ⛓️ Custom Pipelines  
  Chain processing stages like ingestion, enrichment, and transformation (e.g., web galleries or thumbnails).

- 🔄 Automated Daemon Mode  
  Continually monitor a directory (like your import folder) and sort media on the fly.

---

## 🧬 Philosophy

Every image and video is a snapshot of a moment in time and space.  
MONANA is built to respect this core truth by making media archival:

- Predictable: Declare what you want, don’t script how.
- Transparent: No black box. You define the file logic.
- Permanent: Archive like it’s 2034 — and everything still works.

---

## 🚀 Quickstart

Install:

```bash
cargo install monana
```

Run manually on a folder:

```bash
monana run -c ./config/monana.yaml -- /home/nil/import
```

Or start a daemon watcher:

```bash
monana run -c ./config/monana.yaml --watch
```

---

## 🗃️ Example Configuration

Here’s a basic declarative pipeline — YAML format:

```yaml
# Custom action to create low-res images
actions:
  create-low-res:
    command:
      ["magick", "{source.path}", "-resize", "1920x1080>", "{target.path}"]

rulesets:
  Master-Archive:
    input: cmdline
    rules:
      - condition: 'media.type == "video"'
        template: "/mnt/archive/Videos/{time.yyyy}/{time.yyyy}-{time.mm}-{source.original}"
        action: move

      - condition: 'media.type == "image" && space.city == "Madrid"'
        template: "/mnt/archive/Photos/Home/{time.yyyy}/{time.mm}/{source.original}"
        action: move

      - condition: default
        template: "/mnt/archive/Photos/Travel/{space.country}/{space.city}/{time.yyyy}-{time.mm}/{source.original}"
        action: move

  Web-Gallery:
    input: "ruleset:Master-Archive"
    rules:
      - condition: 'media.type == "image"'
        template: "/var/www/images/{time.yyyy}/{source.name}.jpg"
        action: create-low-res
```

---

## 🧰 Built-in Variables

These context variables are available to all templates and conditions:

| Category | Variable            | Description       | Example  |
| -------- | ------------------- | ----------------- | -------- |
| time     | {time.yyyy}         | 4-digit year      | 2024     |
| space    | {space.city}        | City location     | Madrid   |
| source   | {source.name}       | Filename base     | IMG_0001 |
| media    | {media.type}        | Type of file      | image    |
| special  | {special.md5_short} | Unique hash short | a1b2c3d4 |

Full reference available in the Wiki.

---

## 📦 Project Structure

- src/
  - pipeline/ — declarative pipeline engine
  - metadata/ — extraction, parsing, and reverse geocoding
  - actions/ — built-in and custom action invocation
  - config/ — YAML deserialization and validation
- rhai/ — sandboxed scripting hooks (optional)
- tests/ — unit + scenario coverage

---

## 🦀 Built with Rust

Rust gives MONANA:

- blazing performance (great for hundreds of GBs)
- rich CLI ergonomics (clap & structopt)
- memory safety for critical I/O operations

—

## 🔐 Safety by Default

- All user-defined conditions are sandboxed via Rhai
- Custom commands are opt-in and explicitly defined
- Monana never guesses — if metadata is missing, fallback rules apply

---

## 🧪 Development

Clone and try it out locally:

```bash
git clone https://github.com/you/monana.git
cd monana
cargo run -- run -c ./examples/minimal.yaml -- ./test-fixtures/
```

Test suite:

```bash
cargo test
```

---

## 💡 Future Roadmap

- ☁️ Location-aware sync integrations (e.g. Google Takeout reverse-matching)
- 🏷 AI-assisted tagging pipeline
- 🌍 Offline-first reverse geocoding fallback
- 🧞 CLI assistant: Talk to "Monana" interactively
- 📖 GUI front-end? Maybe. Maybe not.

---

## ❤️ Why “Monana”?

It’s a tribute to Monica Geller from Friends — obsessive, organized, relentlessly tidy. And yes, once she awkwardly introduced herself as “Monana.”  
Now, she’s your friendly digital archivist who handles your JPEG drama, so you don’t have to.

---

## 🪪 License

MIT © 2024 You

---

## 📸 Organize your life like Monica would. Automatically.

Questions or praise? File an issue or send a postcard. Monana will file it by country and year.
