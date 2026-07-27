#!/usr/bin/env python3
"""Build a Johnson Controls-branded PDF from a Markdown guide.

Pipeline:  Markdown --python-markdown--> HTML --WeasyPrint--> PDF

Adapted from the Copilot bundle's build_pdf.py. Two changes:

  * The running header/footer strings and the cover footer come from the
    document's YAML frontmatter instead of being hardcoded. assets/jci-brand.css
    is therefore a verbatim copy of the brand stylesheet and can be re-synced
    from it; per-document text is layered on as a second stylesheet, because
    @page margin-box content cannot be varied any other way.

  * There is no "Internal Use Only" default. These guides go to external
    integrators and customers, so the footer must be set deliberately per
    document rather than inherited.

WeasyPrint is required (not Chromium's --print-to-pdf): only a real paged-media
engine implements @page margin boxes and counter(pages), and without those the
running footer overlaps body text instead of living in the page margin.

Usage:
    python build_pdf.py SRC.md [OUT.pdf]
"""
import html as html_mod
import re
import sys
from pathlib import Path

import markdown
from weasyprint import CSS, HTML

ROOT = Path(__file__).resolve().parent
CSS_FILE = ROOT / "assets" / "jci-brand.css"


def parse_frontmatter(text: str) -> dict:
    meta = {}
    m = re.match(r"^\s*---\s*\n(.*?)\n---\s*\n", text, re.DOTALL)
    if m:
        for line in m.group(1).splitlines():
            if ":" in line:
                k, v = line.split(":", 1)
                meta[k.strip()] = v.strip()
    return meta


def strip_frontmatter(text: str) -> str:
    m = re.match(r"^\s*---\s*\n.*?\n---\s*\n", text, re.DOTALL)
    return text[m.end():] if m else text


def split_off_title(text: str):
    """Everything before the first standalone '---' rule is a title block we drop
    (the cover already carries the title)."""
    parts = re.split(r"\n-{3,}\s*\n", text, maxsplit=1)
    return (parts[0], parts[1]) if len(parts) == 2 else ("", text)


def make_cover(meta: dict) -> str:
    return f"""
<section class="cover">
  <img class="logo-img" src="assets/jci-logo.svg" alt="Johnson Controls">
  <div class="block">
    <div class="eyebrow">{meta.get('cover_eyebrow', '')}</div>
    <h1 class="title">{meta.get('cover_title', '')}</h1>
    <p class="sub">{meta.get('cover_sub', '')}</p>
    <div class="meta">{meta.get('cover_meta', '')}</div>
  </div>
  <div class="tagline">The power behind <strong>your mission</strong></div>
  <div class="footerline">{meta.get('cover_footer', '')}</div>
</section>
"""


def page_text_css(meta: dict) -> str:
    """Override the @page margin-box strings for this document.

    CSS `content` takes a quoted string, so anything from frontmatter has to be
    escaped for quotes/backslashes or a stray character silently breaks the
    declaration and the header vanishes. HTML entities are decoded first because
    frontmatter is written for the cover markup, which is HTML.
    """
    def s(key: str) -> str:
        raw = html_mod.unescape(meta.get(key, "")).replace(" ", " ")
        return raw.replace("\\", "\\\\").replace('"', '\\"')

    return (
        "@page { @top-left { content: \"%s\"; } @bottom-left { content: \"%s\"; } }"
        % (s("header_left"), s("footer_left"))
    )


def build(src: Path, out: Path) -> None:
    raw = src.read_text(encoding="utf-8")
    meta = parse_frontmatter(raw)
    _title, body_md = split_off_title(strip_frontmatter(raw))

    body = markdown.markdown(
        body_md,
        extensions=["extra", "sane_lists"],
        output_format="html5",
    )

    doc = (
        '<!DOCTYPE html><html lang="en"><head><meta charset="utf-8">'
        f"<title>{meta.get('cover_title', 'Guide')}</title></head><body>"
        '<div class="page-stripe"></div>'
        f"{make_cover(meta)}"
        f'<main class="content">{body}</main>'
        "</body></html>"
    )

    # Per-document tweaks live in "<source stem>.css" beside the Markdown, so
    # assets/jci-brand.css stays a verbatim copy of the brand stylesheet and can
    # be re-synced from it without losing anything.
    sheets = [CSS(filename=str(CSS_FILE)), CSS(string=page_text_css(meta))]
    doc_css = src.with_suffix(".css")
    if doc_css.exists():
        sheets.append(CSS(filename=str(doc_css)))
        print(f"  + {doc_css.name}")

    HTML(string=doc, base_url=str(ROOT)).write_pdf(str(out), stylesheets=sheets)
    print(f"Wrote {out} ({out.stat().st_size // 1024} KB)")


if __name__ == "__main__":
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    src = Path(sys.argv[1]).resolve()
    out = Path(sys.argv[2]).resolve() if len(sys.argv) > 2 else src.with_suffix(".pdf")
    if not src.exists():
        sys.exit(f"Source not found: {src}")
    build(src, out)
