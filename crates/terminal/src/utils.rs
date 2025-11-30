use std::{env, fs, io::Write, mem, process::Command};

use anyhow::{Result, anyhow};
use ratatui::{
    buffer::{Buffer, Cell},
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span, Text},
};
use syntect_assets::assets::HighlightingAssets;
use tempfile::NamedTempFile;
use tui_syntax_highlight::{Highlighter, syntect::highlighting::Theme};

pub fn highlight_diff_hunk<'a>(diff: &'a str, theme: Theme) -> Text<'a> {
    let diff = textwrap::dedent(diff)
        .lines()
        .map(|line| {
            let Some(c) = line.chars().next() else {
                return format!(" {line}\n");
            };

            match c {
                '+' => format!("+ {}\n", line.split_once("+").unwrap().1),
                '-' => format!("- {}\n", line.split_once("-").unwrap().1),
                _ => format!(" {line}\n"),
            }
        })
        .collect::<String>();

    let mut highlight = syntax_highlight(Some("diff"), &diff, theme);

    for line in &mut highlight.lines {
        line.spans.insert(0, Span::raw("  "));
    }

    highlight
}

pub fn syntax_highlight<'a>(lang: Option<&str>, src: &str, theme: Theme) -> Text<'a> {
    let assets = HighlightingAssets::from_binary();
    let highlighter = Highlighter::new(theme);
    let syntax_set = assets.get_syntax_set().unwrap();
    let syntax = lang
        .and_then(|l| syntax_set.find_syntax_by_token(l))
        .unwrap_or(syntax_set.find_syntax_plain_text());

    highlighter
        .override_background(Color::Reset)
        .line_numbers(false)
        .highlight_lines(src.lines(), syntax, &syntax_set)
        .unwrap()
}

pub fn blit_content(src: &Buffer, viewport: Rect, dest: &mut Buffer, scroll_offset: usize) {
    let blank = Cell::default();
    let src_height = src.area.height as usize;

    for y in 0..viewport.height {
        let dst_y = viewport.y + y;
        let src_y = scroll_offset.saturating_add(y as usize);

        for x in 0..viewport.width {
            let dst_x = viewport.x + x;
            let cell = if src_y < src_height {
                src.cell((x, src_y as u16)).cloned().unwrap_or_default()
            } else {
                blank.clone()
            };
            dest[(dst_x, dst_y)] = cell;
        }
    }
}

pub fn wrap_markdown_body<'a>(body: &'a str, width: usize, theme: Theme) -> Text<'a> {
    #[derive(Debug)]
    enum Part {
        Text(String),
        Code(Option<String>, String),
    }

    let mut current_text = String::with_capacity(body.len());
    let mut parts: Vec<Part> = Vec::new();
    let mut in_code_block = false;
    let mut code_lang = None;

    for line in textwrap::dedent(body).lines() {
        if line.trim_start().starts_with("```") {
            // Process accumulated text before code block
            if !current_text.is_empty() && !in_code_block {
                parts.push(Part::Text(textwrap::wrap(&current_text, width).join("\n")));
                current_text.clear();
            }

            if in_code_block {
                current_text.push('\n');
                current_text.push_str(line);
                parts.push(Part::Code(code_lang.take(), mem::take(&mut current_text)));
                current_text.clear();
                continue;
            } else {
                let lang = line.trim_start().trim_start_matches("```").trim();
                code_lang = (!lang.is_empty()).then_some(lang.to_owned());
            }

            in_code_block = !in_code_block;

            if !current_text.is_empty() {
                current_text.push('\n');
            }
            current_text.push_str(line);
        } else if in_code_block {
            // In code block: add line as-is without wrapping
            if !current_text.is_empty() {
                current_text.push('\n');
            }
            current_text.push_str(line);
        } else {
            // Not in code block: accumulate text for wrapping
            if !current_text.is_empty() {
                current_text.push('\n');
            }
            current_text.push_str(line);
        }
    }

    if !current_text.is_empty() {
        parts.push(Part::Text(textwrap::wrap(&current_text, width).join("\n")));
    }

    let mut result = Text::default();

    for part in parts {
        let lines = match part {
            Part::Text(txt) => {
                let md = tui_markdown::from_str(&txt);
                result.extend(md.lines);
            }
            Part::Code(lang, txt) => {
                let code = syntax_highlight(lang.as_deref(), &txt.clone(), theme.clone());
                result.extend(code.lines);
            }
        };
    }

    result
}

#[cfg(test)]
mod tests {
    use crate::utils::wrap_markdown_body;

    #[test]
    fn test_wrap_markdown_body() {
        wrap_markdown_body(
            r#"
            # Torum Iovi delphines miseroque

            ## Ignibus solet adspicit pennas

            Lorem markdownum facti, **erat caelo te** aliquis aspicit curvi nec officio? Gravis videri una, et **ripa** accipitris inquit teretesque dum, *o*!

            Per ille patet aethere percepitque reddat magistris convivia petere, trepidantem rogata, fluviumque fit matris! Scire *voveam* frondes Hecaten, venit cristae versasque licet ei [mater motu arcus](#ignibus-solet-adspicit-pennas) reddere sed. Pavidamque manantem mihi `copy` inter quod captivarum luridus texit: meae, nubifer, qui!

            ## Et succedit inventum arma

            Ne refugitque exsistunt *amasse*. Molior conlapsosque haec et ego, iussum silva, nostras ut nefasque capit dixerat pollice os vitibus aduncis bis.

            - Est valet non Argos exsultat conditus sacra
            - A Circes iuvenum in redeat sistitur se
            - Adest Iovi non sed protinus parte aperite
            - Nec virorum pudore
            - Rapidum malus

            ## Aut accepisse quae neque

            Hodierna verba probat scitis collibus olor cinis caecum non et Troiana, Tyros! Aliter qui extrema longum [prohibente](#et-succedit-inventum-arma) lanas hamatis, nox `trackback` et Achilles. Nescius [novi](#et-succedit-inventum-arma) vinoque Capysque ille Hoc. Dat reus crinibus animae. Et voti.

            ## Paternis si umentia ingemit

            In eget duritiem Piraeaque vult quo quam, odoratas matrisque molli obsuntque vetus animus mundi clipeum? Nova in [efflant Latona](#et-succedit-inventum-arma); esset solvi aequor thalamos occupat artes **voco**, ruricolae, flamma! Capillos aera posset parens. Lacertis tollens reccidat sanior?

            1. Coepere capellas noviens nata ille mariti te
            2. Ab simplex perdite testata oscula funduntque ad
            3. Coacti sustulerat preces

            ## Mori silentum loca amensque

            Matrona perdere lancea moratum, fumida hic credere dirum sceptri percussit solebat traxisse et teneras `fileDomainJre` concordant. **Pactolides** saltem, edax repetebam suos undas viaque compos captus.

            ```cpp
            if (780067 + access_ssl_it - shell.trim(4, 3, impactKbpsGigabit) + 2 - scraping) {
                clean_css.ppgaWww += taskPplSector + biometricsModemVirtual;
            }
            kilohertz_clean_dv(orientation);
            data.touchscreenSubdirectoryThread(unicode(executable_intellectual_secondary, toslinkBitrateMap(formatOfAndroid, zebibyte_web_parameter, hardware), intranetInterpreter.rateColumnVga.dualToken(sms, uri_pmu_hypertext, eps)), drive + registry, web_e(addressSocketC, commerce));
            ```

            ## Paternis si umentia ingemit

            In eget duritiem Piraeaque vult quo quam, odoratas matrisque molli obsuntque vetus animus mundi clipeum? Nova in [efflant Latona](#et-succedit-inventum-arma); esset solvi aequor thalamos occupat artes **voco**, ruricolae, flamma! Capillos aera posset parens. Lacertis tollens reccidat sanior?

            1. Coepere capellas noviens nata ille mariti te
            2. Ab simplex perdite testata oscula funduntque ad
            3. Coacti sustulerat preces

            ## Mori silentum loca amensque

            Matrona perdere lancea moratum, fumida hic credere dirum sceptri percussit solebat traxisse et teneras `fileDomainJre` concordant. **Pactolides** saltem, edax repetebam suos undas viaque compos captus.

            "#,
            80,
        );
    }
}

pub fn stylize_block(text: &mut Text, style: Style) {
    let line_len = text.lines.len();
    for (index, line) in text.iter_mut().enumerate() {
        let block = match index {
            0 if line_len == 1 => "",
            0 if line_len > 1 => "╭ ",
            _ if index + 1 == line_len => "╰ ",
            _ => "│ ",
        };

        line.spans.insert(0, Span::raw(block).style(style));
    }
}

pub fn suspend_for_editor(initial_contents: String) -> Result<String> {
    // leave_terminal()?;

    let editor = env::var("EDITOR").unwrap_or_else(|_| "vim".into());

    let mut tempfile = NamedTempFile::new()?;
    tempfile.write_all(initial_contents.as_bytes())?;
    tempfile.flush()?;

    let status = Command::new(&editor).arg(tempfile.path()).status()?;
    if !status.success() {
        return Err(anyhow!("editor exited with {}", status));
    }

    let body = fs::read_to_string(tempfile.path())?;

    // enter_terminal()?;

    Ok(body)
}
