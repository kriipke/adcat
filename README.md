> [!IMPORTANT]
> This is a fork of [mdcat](https://github.com/swsnr/mdcat). The upstream project is no longer maintained.

# xcat

Fancy `cat` for Markdown (that is, [CommonMark][]) and AsciiDoc:

```
$ xcat sample.md
$ xcat sample.adoc
```

![xcat showcase with different colour themes][sxs]

xcat in [WezTerm], with "One Light (base16)", "Gruvbox Light", and "Darcula
(base16)" (from left to right), and [JetBrains Mono] as font.

[CommonMark]: http://commonmark.org
[Solarized]: http://ethanschoonover.com/solarized
[dracula]: https://draculatheme.com/iterm/
[wezterm]: https://wezfurlong.org/wezterm/
[JetBrains Mono]: https://www.jetbrains.com/lp/mono/
[sxs]: ./screenshots/side-by-side.png

## Features

`xcat` works best with [iTerm2], [WezTerm], and [kitty], and a good terminal font with italic characters.
Then it

* nicely renders all basic CommonMark syntax,
* renders AsciiDoc documents (`.adoc`, `.asciidoc`) with equivalent formatting support,
* highlights code blocks with [syntect],
* shows [links][osc8], and also images inline in supported terminals (see above, where "Rust" is a clickable link!),
* adds jump marks for headings in [iTerm2] (jump forwards and backwards with <key>⇧⌘↓</key> and <key>⇧⌘↑</key>).

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

1) xcat requires that the terminal supports strikethrough formatting and [inline links][osc8].
    This includes most modern terminal emulators, such as Windows Terminal, KDE Konsole, or anything based on VTE, GNOME's terminal emulation library.
    But xcat likely won't work well on old terminals that lack these features (e.g. the Linux text console).
2) SVG images are rendered with [resvg], see [SVG support].

Not supported:

* CommonMark extension for footnotes.
* Inline markup and text wrapping in table cells.
* AsciiDoc tables, include directives, and conditional preprocessing.

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

Try `xcat --help` or read the [xcat(1)](./xcat.1.adoc) manpage.

## Installation

* [Release binaries](https://github.com/kriipke/xcat/releases/) built on Github Actions.
  - These binaries are build from Git source on Github actions; you find provenance attestations at <https://github.com/kriipke/xcat/attestations>.
* 3rd party packages at [Repology](https://repology.org/project/mdcat/versions)
* You can also build `xcat` manually with `cargo install --path .` (see below for details).

`xcat` can be linked or copied to `xless`; if invoked as `xless` it automatically uses pagination.

## Building

Run `cargo build --release`.

Building requires `libcurl`.

## Packaging

When packaging `xcat` you may wish to include the following additional artifacts:

- A symlink or hardlink from `xless` to `xcat` (see above).
- Shell completions for relevant shells, by invoking `xcat --completions` after building, e.g.

  ```console
  $ xcat --completions fish > /usr/share/fish/vendor_completions.d/xcat.fish
  $ xcat --completions bash > /usr/share/bash-completion/completions/xcat
  $ xcat --completions zsh > /usr/share/zsh/site-functions/_xcat
  # Same for xless if you include it
  $ xless --completions fish > /usr/share/fish/vendor_completions.d/xless.fish
  $ xless --completions bash > /usr/share/bash-completion/completions/xless
  $ xless --completions zsh > /usr/share/zsh/site-functions/_xless
  ```

- A build of the man page `xcat.1.adoc`, using [AsciiDoctor]:

  ```console
  $ asciidoctor -b manpage -a reproducible -o /usr/share/man/man1/xcat.1 xcat.1.adoc
  $ gzip /usr/share/man/man1/xcat.1
  # If you include a xless as above, you may also want to support man xless
  $ ln -s xcat.1.gz /usr/share/man/man1/xless.1.gz
  ```

[AsciiDoctor]: https://asciidoctor.org/

## Troubleshooting

`xcat` can output extensive tracing information when asked to.
Run `xcat` with `$XCAT_LOG=trace` for complete tracing information, or with `$XCAT_LOG=xcat::render=trace` to trace only rendering.

## License

Copyright Sebastian Wiesner <sebastian@swsnr.de>

Binaries are subject to the terms of the Mozilla Public
License, v. 2.0, see [LICENSE](LICENSE).

Most of the source is subject to the terms of the Mozilla Public
License, v. 2.0, see [LICENSE](LICENSE), unless otherwise noted;
some files are subject to the terms of the Apache 2.0 license,
see <http://www.apache.org/licenses/LICENSE-2.0>
