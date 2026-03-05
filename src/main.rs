mod rain;
mod cli;
mod app;
mod chat;
mod input;
mod gateway;
mod settings;
mod effects;
mod persist;
mod mood;
mod mood_tag;
#[cfg(test)]
mod test;

use clap::Parser;
use crossterm::event::{Event, EventStream};
use ezemoji::CharGroup;
use futures::StreamExt;
use std::str::FromStr;
use std::time::Duration;

// Re-exports needed by cli.rs (uses super::{AUTHOR, Direction, MAXSPEED, MINSPEED})
pub use rain::{Direction, MAXSPEED, MINSPEED};
// Re-exports used by test.rs
#[cfg(test)]
pub(crate) use rain::gen_shade_color;
#[cfg(test)]
pub(crate) use rain::Rain;


use crate::cli::Grouping;
use app::{App, AppMode};

const AUTHOR: &str = "
▞▀▖       ▌        ▞▀▖▞▀▖▞▀▖▛▀▘
▌  ▞▀▖▌  ▌▛▀▖▞▀▖▌ ▌▚▄▘▙▄  ▗▘▙▄
▌ ▖▌ ▌▐▐▐ ▌ ▌▌ ▌▚▄▌▌ ▌▌ ▌▗▘ ▖ ▌
▝▀ ▝▀  ▘▘ ▀▀ ▝▀ ▗▄▘▝▀ ▝▀ ▀▀▘▝▀
Email: cowboy8625@protonmail.com
";

fn update_settings_with_config(settings: &mut cli::Cli) {
    let Some(config) = cli::load_config() else {
        return;
    };
    if let Some(shade) = config.shade {
        settings.shade = shade;
    }
    if let Some(color) = config.color {
        settings.color = color;
    }
    if let Some(shade_gradient) = config.shade_gradient {
        settings.shade_gradient = shade_gradient;
    }
    if let Some(head) = config.head {
        settings.head = head;
    }
    if let Some(direction) = config.direction {
        settings.direction = direction;
    }
    if let Some(speed) = config.speed {
        settings.speed = speed;
    }
    if let Some(display_group) = config.display_group {
        settings.display_group = display_group;
    }
    let Some(name) = config.group else {
        return;
    };
    match CharGroup::from_str(name.as_str()) {
        Ok(group) => {
            settings.group = Grouping::from(group);
        }
        Err(_) => {
            if let Some(group) = config.custom.get(name.as_str()) {
                settings.group = Grouping::from(group.clone());
                return;
            }
            eprintln!("group not found {name}");
        }
    }
}

fn main() -> std::io::Result<()> {
    let mut settings = cli::Cli::parse();
    update_settings_with_config(&mut settings);
    // Restore persisted UI settings (overridden by explicit CLI args)
    persist::apply(&persist::load(), &mut settings);

    if settings.display_group {
        let extra_width = matches!(
            settings.group.name(),
            ezemoji::GroupKind::Custom("OpenSource")
                | ezemoji::GroupKind::Custom("ProgrammingLanguages")
        );
        match settings.group {
            Grouping::EzEmoji(group) => {
                for char in group.iter() {
                    print!("{char}");
                    if extra_width {
                        print!(" ");
                    }
                }
            }
            Grouping::Custom(group) => {
                for range in group.range.iter() {
                    for cp in range.clone() {
                        print!("{}", std::char::from_u32(cp).unwrap_or('🤦'));
                    }
                }
            }
        }
        return Ok(());
    }

    // Start tokio runtime for async event loop
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async_main(settings))
}

async fn async_main(settings: cli::Cli) -> std::io::Result<()> {
    let mut terminal = ratatui::init();
    let size = terminal.size()?;
    let mut app = App::new(size.width, size.height, settings);

    let result = run_app(&mut terminal, &mut app).await;
    ratatui::restore();
    result
}

async fn run_app(
    terminal: &mut ratatui::Terminal<impl ratatui::backend::Backend>,
    app: &mut App,
) -> std::io::Result<()> {
    let tick_rate = Duration::from_millis(50);
    let mut tick_interval = tokio::time::interval(tick_rate);
    let mut event_stream = EventStream::new();

    loop {
        tokio::select! {
            _ = tick_interval.tick() => {
                app.tick();
                app.process_gateway_actions();
            }
            event = event_stream.next() => {
                match event {
                    Some(Ok(Event::Key(key))) => {
                        app.handle_key(key);
                    }
                    Some(Ok(Event::Resize(w, h))) => {
                        app.rebuild_rain(w, h);
                    }
                    Some(Err(_)) => break,
                    None => break,
                    _ => {}
                }
            }
            // Future: gateway actions will be selected here
        }

        if app.mode == AppMode::Exiting {
            break;
        }

        terminal.draw(|frame| {
            app.draw(frame);
        })?;
    }

    Ok(())
}
