use anyhow::{bail, Context, Result};
use clap::Parser;
use crossterm::event::{self, KeyCode, KeyModifiers};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::palette::tailwind;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Gauge, Padding, Paragraph};
use rodio::Source;

#[derive(Parser)]
#[command(version, about, long_about = Some("Mix up sounds from directories"))]
struct Args {
    #[arg(default_values_t = vec!["sounds".to_string()])]
    dirs: Vec<String>,
    #[arg(long, default_value_t = 25)]
    volume_granularity: u8,
    #[arg(long, default_value_t = 0)]
    start_volume: u8,
}

struct Stream {
    filename: String,
    sink: rodio::Sink,
    volume: u8,
}

struct State {
    streams: Vec<Stream>,
    volume_granularity: u8,
    selected: usize,
    paused: bool,
}

fn add_path(
    state: &mut State,
    volume: u8,
    mixer: &rodio::mixer::Mixer,
    path: &std::path::Path,
) -> Result<()> {
    let file = std::fs::File::open(path)?;
    let sink = rodio::Sink::connect_new(mixer);
    let decoded = rodio::Decoder::try_from(file)?;
    sink.append(decoded.repeat_infinite());
    let start_volume = volume as f32 / state.volume_granularity as f32;
    sink.set_volume(start_volume);
    if start_volume == 0. {
        sink.pause();
    }
    let filename = path
        .file_stem()
        .context("no filename?")?
        .to_string_lossy()
        .into();
    state.streams.push(Stream {
        filename,
        sink,
        volume,
    });
    Ok(())
}

impl State {
    fn stream(&mut self, update: isize) {
        let len = self.streams.len() as isize;
        self.selected = ((self.selected as isize + len + update) % len) as usize
    }
    fn volume(&mut self, update: i8) {
        let stream = &mut self.streams[self.selected];
        stream.volume = stream
            .volume
            .saturating_add_signed(update)
            .min(self.volume_granularity);
        if stream.volume == 0 {
            stream.sink.pause();
        } else {
            stream
                .sink
                .set_volume(stream.volume as f32 / self.volume_granularity as f32);
            stream.sink.play();
        }
    }
    fn all_volume(&mut self, update: i8) {
        let old_selected = self.selected;
        for i in 0..self.streams.len() {
            self.selected = i;
            self.volume(update);
        }
        self.selected = old_selected;
    }
    fn toggle_playpause(&mut self) {
        self.paused = !self.paused;
        let apply = if self.paused {
            rodio::Sink::pause
        } else {
            rodio::Sink::play
        };
        for i in 0..self.streams.len() {
            apply(&self.streams[i].sink);
        }
    }
    fn run(mut self, terminal: &mut ratatui::DefaultTerminal) -> Result<()> {
        loop {
            terminal.draw(|frame| self.render(frame))?;

            if let Some(key) = event::read()?.as_key_press_event() {
                let ctrl_pressed = key.modifiers.contains(KeyModifiers::CONTROL);
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Char('j') | KeyCode::Down => self.stream(1),
                    KeyCode::Char('k') | KeyCode::Up => self.stream(-1),
                    KeyCode::Char('l') | KeyCode::Right if ctrl_pressed => self.all_volume(1),
                    KeyCode::Char('h') | KeyCode::Left if ctrl_pressed => self.all_volume(-1),
                    KeyCode::Char('l') | KeyCode::Right => self.volume(1),
                    KeyCode::Char('h') | KeyCode::Left => self.volume(-1),
                    KeyCode::Char(' ') => self.toggle_playpause(),
                    _ => {}
                }
            }
        }
    }
    fn render(&self, frame: &mut ratatui::Frame) {
        use Constraint::{Length, Min, Ratio};
        let layout = Layout::vertical([Length(2), Min(5)]);
        let [header, main] = frame.area().layout(&layout);

        // header
        let title = if self.paused {
            "Fuurin (paused)"
        } else {
            "Fuurin"
        };
        let widget = Paragraph::new(title).bold().fg(tailwind::SLATE.c200);
        frame.render_widget(widget, header);

        // scrollbars
        // XXX get total height vs. len and only print max n lines centered around focused item if
        // it doesn't fit, assume all fit for now
        // also make sure all segments are identical, this doesn't do it.
        let num = self.streams.len();
        let mut layout = Vec::with_capacity(num);
        for _ in 0..num {
            layout.push(Ratio(2, num as u32));
        }
        let layout = Layout::vertical(&layout);
        let lines = main.layout_vec(&layout);
        for (i, line) in lines.into_iter().enumerate() {
            let stream = &self.streams[i];
            let title = Line::from(stream.filename.as_str()).centered();
            let title = Block::new()
                .borders(Borders::NONE)
                .padding(Padding::vertical(0))
                .title(title)
                .fg(tailwind::SLATE.c200);
            let gauge = Gauge::default()
                .block(title)
                .gauge_style(tailwind::BLUE.c800)
                .ratio(stream.volume as f64 / self.volume_granularity as f64)
                .use_unicode(true);
            frame.render_widget(gauge, line);
        }
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    let mut state = State {
        streams: vec![],
        volume_granularity: args.volume_granularity,
        selected: 0,
        paused: false,
    };

    let mut stream_handle = rodio::OutputStreamBuilder::open_default_stream()?;
    stream_handle.log_on_drop(false);
    let mixer = stream_handle.mixer();

    // add all the files
    for dir in &args.dirs {
        let readdir = match std::fs::read_dir(dir) {
            Ok(readdir) => readdir,
            Err(e) if e.kind() == std::io::ErrorKind::NotADirectory => {
                let path = std::path::Path::new(&dir);
                if let Err(e) = add_path(&mut state, args.start_volume, mixer, path) {
                    println!("Could not add {path:?}: {e:?}");
                }
                continue;
            }
            Err(e) => {
                println!("Could not read {dir}: {e:?}");
                continue;
            }
        };
        for f in readdir {
            let Ok(f) = f else {
                continue;
            };
            let path = f.path();
            match path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or_default()
            {
                "ogg" | "mp3" | "wav" => (),
                _ => continue,
            }
            if let Err(e) = add_path(&mut state, args.start_volume, mixer, &path) {
                println!("Could not add {path:?}: {e:?}");
            }
        }
    }
    if state.streams.is_empty() {
        bail!("No media found");
    }

    ratatui::run(|terminal| state.run(terminal))
}
