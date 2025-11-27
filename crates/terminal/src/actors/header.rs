use std::{fmt::Write, sync::Arc};

use anyhow::Result;
use chrono_humanize::Humanize;
use crossterm::style::Stylize;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    text::Line,
    widgets::{StatefulWidgetRef, WidgetRef},
};

use crate::{actors::Actor, app::Context};

#[derive(Debug)]
pub struct Header;

impl Actor for Header {
    fn dirty(&self) -> bool {
        false
    }
}

impl StatefulWidgetRef for Header {
    type State = Arc<Context>;

    fn render_ref(&self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let line = Line::from(format!(
            "{}",
            state.github.pr.title.as_deref().unwrap_or("Pull Request")
        ));

        line.render_ref(area, buf);

        // let mut pr_link = String::new();
        // write!(pr_link, "(")?;
        // match ctx.github.pr.html_url.as_ref() {
        //     Some(url) => hyperlink(&mut pr_link, format!("#{}", ctx.github.pr.number), url),
        //     None => write!(pr_link, "#{}", ctx.github.pr.number),
        // }?;
        // write!(pr_link, ")")?;

        // write!(buf, " {}", pr_link.with(ctx.color_scheme.muted.into()))?;

        // writeln!(buf)?;

        // if let Some(author) = ctx.github.pr.user.as_ref() {
        //     write!(
        //         buf,
        //         "  {}",
        //         author.login.as_str().with(ctx.color_scheme.author.into())
        //     )?;
        // }

        // if let Some(updated_at) = ctx.github.pr.updated_at.as_ref() {
        //     write!(
        //         buf,
        //         " {} {}",
        //         "·".with(ctx.color_scheme.muted.into()),
        //         updated_at.humanize().with(ctx.color_scheme.muted.into())
        //     )?;
        // }

        // if let Some(count) = ctx.github.pr.changed_files {
        //     write!(
        //         buf,
        //         " {} {}",
        //         "·".with(ctx.color_scheme.muted.into()),
        //         {
        //             let mut s = String::new();
        //             write!(
        //                 s,
        //                 "{} file{} changed",
        //                 count,
        //                 if count == 1 { "" } else { "s" }
        //             )?;
        //             s
        //         }
        //         .with(ctx.color_scheme.muted.into())
        //     )?;
        // }

        // writeln!(buf)?;

        // Ok(())
    }
}
