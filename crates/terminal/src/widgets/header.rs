use std::fmt::Write;

use anyhow::Result;
use chrono_humanize::Humanize;
use crossterm::style::Stylize;

use crate::{app::Context, utils::hyperlink, widgets::Widget};

#[derive(Debug)]
pub struct Header;

impl Widget for Header {
    fn render(&mut self, buf: &mut String, app: &Context) -> Result<()> {
        write!(
            buf,
            "  {}",
            app.github
                .pr
                .title
                .as_deref()
                .unwrap_or("Pull Request")
                .with(app.color_scheme.pr_title.into())
                .bold(),
        )?;

        let mut pr_link = String::new();
        write!(pr_link, "(")?;
        match app.github.pr.html_url.as_ref() {
            Some(url) => hyperlink(&mut pr_link, format!("#{}", app.github.pr.number), url),
            None => write!(pr_link, "#{}", app.github.pr.number),
        }?;
        write!(pr_link, ")")?;

        write!(buf, " {}", pr_link.with(app.color_scheme.muted.into()))?;

        writeln!(buf)?;

        if let Some(author) = app.github.pr.user.as_ref() {
            write!(
                buf,
                "  {}",
                author.login.as_str().with(app.color_scheme.author.into())
            )?;
        }

        if let Some(updated_at) = app.github.pr.updated_at.as_ref() {
            write!(
                buf,
                " {} {}",
                "·".with(app.color_scheme.muted.into()),
                updated_at.humanize().with(app.color_scheme.muted.into())
            )?;
        }

        if let Some(count) = app.github.pr.changed_files {
            write!(
                buf,
                " {} {}",
                "·".with(app.color_scheme.muted.into()),
                {
                    let mut s = String::new();
                    write!(
                        s,
                        "{} file{} changed",
                        count,
                        if count == 1 { "" } else { "s" }
                    )?;
                    s
                }
                .with(app.color_scheme.muted.into())
            )?;
        }

        writeln!(buf)?;

        Ok(())
    }

    fn dirty(&self) -> bool {
        false
    }
    // fn tick(&mut self, _app: &App, _event: &Event) -> Result<Tick> {
    //     Ok(Tick::Noop)
    // }

    // fn render(&self, buf: &mut String, app: &App) -> Result<()> {
    //     write!(
    //         buf,
    //         "  {}",
    //         app.github
    //             .pr
    //             .title
    //             .as_deref()
    //             .unwrap_or("Pull Request")
    //             .with(app.color_scheme.pr_title.into())
    //             .bold(),
    //     )?;

    //     let mut pr_link = String::new();
    //     write!(pr_link, "(")?;
    //     match app.github.pr.html_url.as_ref() {
    //         Some(url) => hyperlink(&mut pr_link, format!("#{}", app.github.pr.number), url),
    //         None => write!(pr_link, "#{}", app.github.pr.number),
    //     }?;
    //     write!(pr_link, ")")?;

    //     write!(buf, " {}", pr_link.with(app.color_scheme.muted.into()))?;

    //     writeln!(buf)?;

    //     if let Some(author) = app.github.pr.user.as_ref() {
    //         write!(
    //             buf,
    //             "  {}",
    //             author.login.as_str().with(app.color_scheme.author.into())
    //         )?;
    //     }

    //     if let Some(updated_at) = app.github.pr.updated_at.as_ref() {
    //         write!(
    //             buf,
    //             " {} {}",
    //             "·".with(app.color_scheme.muted.into()),
    //             updated_at.humanize().with(app.color_scheme.muted.into())
    //         )?;
    //     }

    //     if let Some(count) = app.github.pr.changed_files {
    //         write!(
    //             buf,
    //             " {} {}",
    //             "·".with(app.color_scheme.muted.into()),
    //             {
    //                 let mut s = String::new();
    //                 write!(
    //                     s,
    //                     "{} file{} changed",
    //                     count,
    //                     if count == 1 { "" } else { "s" }
    //                 )?;
    //                 s
    //             }
    //             .with(app.color_scheme.muted.into())
    //         )?;
    //     }

    //     writeln!(buf)?;

    //     Ok(())
    // }
}
