> [!IMPORTANT]
> `adcat` is a fork of [mdcat](https://github.com/swsnr/mdcat) that adds first-class [AsciiDoc][] rendering while keeping the original Markdown support intact. The upstream `mdcat` project is no longer maintained.

# adcat

A `cat` for **AsciiDoc** that also speaks **Markdown**.

`mdcat` already rendered CommonMark beautifully in the terminal. `adcat` is a fork that teaches it AsciiDoc — `.adoc` and `.asciidoc` files render with the same fidelity (formatting, code highlighting, inline images, hyperlinks) as Markdown, using [acdc-parser][] for AsciiDoc and `pulldown-cmark` for Markdown. Markdown support is preserved verbatim from upstream; nothing was traded away.

```
$ adcat README.adoc      # primary use case: AsciiDoc
$ adcat sample.md        # still works exactly like mdcat
```

![adcat showcase with different colour themes][sxs]

adcat in [WezTerm], with "One Light (base16)", "Gruvbox Light", and "Darcula
(base16)" (from left to right), and [JetBrains Mono] as font.

[AsciiDoc]: https://gitlab.eclipse.org/eclipse/asciidoc-lang/asciidoc-lang/-/blob/main/spec/outline.adoc?ref_type=heads
[CommonMark]: http://commonmark.org
[acdc-parser]: https://crates.io/crates/acdc-parser
[wezterm]: https://wezfurlong.org/wezterm/
[JetBrains Mono]: https://www.jetbrains.com/lp/mono/
[sxs]: ./screenshots/side-by-side.png

## What's new in adcat (vs. mdcat)

* **AsciiDoc rendering.** Files ending in `.adoc` or `.asciidoc` are parsed with [acdc-parser][] and rendered to the terminal with the same pipeline as Markdown — sections, lists, tables, code blocks, admonitions, inline macros, images.
* **AsciiDoc preprocessing.** File-backed `.adoc` documents go through the parser's preprocessor, so `include::` directives and conditional blocks (`ifdef`, `ifndef`, `ifeval`) work as expected.
* **Table fidelity for AsciiDoc.** Header/footer rows and cell column spans round-trip through an internal marker convention shared with the bundled renderer.
* **Markdown unchanged.** All [CommonMark][] features that worked in `mdcat` still work in `adcat`. If you point `adcat` at a `.md` file, you get the same output you'd get from upstream `mdcat`.

## Features

`adcat` works best with [iTerm2], [WezTerm], and [kitty], and a good terminal font with italic characters. It

* renders AsciiDoc (`.adoc`, `.asciidoc`) with sections, lists, tables, callouts, admonitions, and inline macros,
* renders all basic CommonMark Markdown syntax,
* highlights code blocks with [syntect],
* shows [links][osc8] and inline images in supported terminals (see above, where "Rust" is a clickable link),
* adds jump marks for headings in [iTerm2] (jump with <key>⇧⌘↓</key> / <key>⇧⌘↑</key>).

| Terminal                   |  Basic syntax | Syntax highlighting | Images | Jump marks |
| :------------------------- | :-----------: | :-----------------: | :----: | :--------: |
| Basic ANSI¹                | ✓             | ✓                   |        |            |
| Windows 10 console         | ✓             | ✓                   |        |            |
| [Terminology]              | ✓             | ✓                   | ✓      |            |
| [iTerm2]                   | ✓             | ✓                   | ✓²     | ✓          |
| [kitty]                    | ✓             | ✓                   | ✓²     |            |
| [WezTerm]                  | ✓             | ✓                   | ✓²     |            |
| [VSCode]                   | ✓             | ✓                   | ✓²     |            |
| [Ghostty]                  | ✓             | ✓                   | ✓²     |            |

1) adcat requires that the terminal supports strikethrough formatting and [inline links][osc8]. This includes most modern terminal emulators (Windows Terminal, KDE Konsole, anything based on VTE). It likely won't work well on old terminals that lack these features (e.g. the Linux text console).
2) SVG images are rendered with [resvg], see [SVG support].

Not supported:

* CommonMark extension for footnotes.
* Inline markup and text wrapping in table cells (Markdown or AsciiDoc).
* Some advanced AsciiDoc processor semantics are still rendered approximately.
* Standard-input AsciiDoc cannot resolve relative `include::` directives (no source path to anchor against).

[syntect]: https://github.com/trishume/syntect
[osc8]: https://gist.github.com/egmontkob/eb114294efbcd5adb1944c9f3cb5feda
[Terminology]: http://terminolo.gy
[iterm2]: https://www.iterm2.com
[WezTerm]: https://wezfurlong.org/wezterm/
[kitty]: https://sw.kovidgoyal.net/kitty/
[resvg]: https://github.com/RazrFalcon/resvg
[SVG support]: https://github.com/RazrFalcon/resvg#svg-support
[VSCode]: https://code.visualstudio.com/
[Ghostty]: https://mitchellh.com/ghostty

## Usage

Try `adcat --help` or read the [adcat(1)](./adcat.1.adoc) manpage.

`adcat` selects the renderer by file extension: `.adoc` / `.asciidoc` go through the AsciiDoc pipeline, everything else (including `-` for stdin) is treated as Markdown.

## Installation

* [Release binaries](https://github.com/kriipke/adcat/releases/) built on GitHub Actions. Provenance attestations at <https://github.com/kriipke/adcat/attestations>.
* You can also build `adcat` manually with `cargo install --path .` (see below).

`adcat` can be linked or copied to `adless`; if invoked as `adless` it automatically uses pagination.

## Building

Run `cargo build --release`.

Building requires `libcurl`.

## Packaging

When packaging `adcat` you may wish to include the following additional artifacts:

- A symlink or hardlink from `adless` to `adcat` (see above).
- Shell completions for relevant shells, by invoking `adcat --completions` after building, e.g.

  ```console
  $ adcat --completions fish > /usr/share/fish/vendor_completions.d/adcat.fish
  $ adcat --completions bash > /usr/share/bash-completion/completions/adcat
  $ adcat --completions zsh > /usr/share/zsh/site-functions/_adcat
  # Same for adless if you include it
  $ adless --completions fish > /usr/share/fish/vendor_completions.d/adless.fish
  $ adless --completions bash > /usr/share/bash-completion/completions/adless
  $ adless --completions zsh > /usr/share/zsh/site-functions/_adless
  ```

- A build of the man page `adcat.1.adoc`, using [AsciiDoctor]:

  ```console
  $ asciidoctor -b manpage -a reproducible -o /usr/share/man/man1/adcat.1 adcat.1.adoc
  $ gzip /usr/share/man/man1/adcat.1
  # If you include adless as above, you may also want to support man adless
  $ ln -s adcat.1.gz /usr/share/man/man1/adless.1.gz
  ```

[AsciiDoctor]: https://asciidoctor.org/

## Troubleshooting

`adcat` can output extensive tracing information when asked to. Run `adcat` with `$ADCAT_LOG=trace` for complete tracing, or `$ADCAT_LOG=adcat::render=trace` to trace only rendering.

## License

Copyright Sebastian Wiesner <sebastian@swsnr.de> and contributors (upstream `mdcat`); AsciiDoc additions copyright the `adcat` contributors.

Binaries are subject to the terms of the Mozilla Public License, v. 2.0, see [LICENSE](LICENSE).

Most of the source is subject to the terms of the Mozilla Public License, v. 2.0, see [LICENSE](LICENSE), unless otherwise noted; some files are subject to the terms of the Apache 2.0 license, see <http://www.apache.org/licenses/LICENSE-2.0>.
