use anyhow::{bail, Context, Result};
use clap::Parser;
use crossterm::event::{self, KeyCode, KeyModifiers};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::palette::tailwind;
use ratatui::style::{Style, Stylize};
use ratatui::widgets::{Block, Borders, Gauge, Paragraph};
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
    longest_filename: usize,
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
    let filename: String = path
        .file_stem()
        .context("no filename?")?
        .to_string_lossy()
        .into();
    if filename.len() > state.longest_filename {
        state.longest_filename = filename.len();
    }
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
            // XXX rodio uses too much cpu even when all sinks are paused,
            // try to find a better way to pause the whole mixer/output...
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
        use Constraint::{Length, Max, Min};
        let layout = Layout::vertical([Length(2), Max(5), Min(5), Max(5)]);
        let [header, _centering_pre, main, _centering_post] = frame.area().layout(&layout);

        // header
        let title = if self.paused {
            "Fuurin (paused)"
        } else {
            "Fuurin"
        };
        let widget = Paragraph::new(title).bold().fg(tailwind::SLATE.c200);
        frame.render_widget(widget, header);

        // scrollbars

        let usable_height = main.height;
        let num = self.streams.len() as u16;
        let lines_per_widget = usable_height / num;
        let mut layout = Vec::with_capacity(num as usize);
        for _ in 0..num.min(usable_height) {
            layout.push(Length(lines_per_widget.max(1)));
        }
        let layout = Layout::vertical(&layout);
        let lines = main.layout_vec(&layout);
        for (i, line) in lines.into_iter().enumerate() {
            let i = if lines_per_widget != 0 {
                i
            } else {
                // we don't have enough lines, center around selected
                match self.selected as u16 {
                    s if s < usable_height / 2 => i,
                    s if num - s - 1 < usable_height / 2 => {
                        // i goes from 0 to usable_height - 1 so
                        // we want i == usable_height - 1 => num - 1
                        i + num as usize - usable_height as usize
                    }
                    _ => i + self.selected - usable_height as usize / 2,
                }
            };

            let stream = &self.streams[i];
            let ratio = stream.volume as f64 / self.volume_granularity as f64;
            let layout = Layout::horizontal([
                Length(10),
                Length(self.longest_filename as u16 + 7 /* ' 100.0% ' */),
                Min(5),
                Length(10),
            ]);
            let [_padding_left, label_area, gauge_area, _padding_right] = line.layout(&layout);

            let title = format!(
                "{:width$} {:3.0}%",
                stream.filename,
                ratio * 100.,
                width = self.longest_filename
            );
            let label = Block::new()
                .borders(Borders::NONE)
                .title(title)
                .fg(tailwind::SLATE.c200);
            frame.render_widget(label, label_area);

            let filled = if i == self.selected {
                tailwind::BLUE.c300
            } else if i % 2 == 0 {
                tailwind::BLUE.c400
            } else {
                tailwind::BLUE.c500
            };
            let unfilled = if i == self.selected {
                tailwind::BLUE.c700
            } else if i % 2 == 0 {
                tailwind::BLUE.c800
            } else {
                tailwind::BLUE.c900
            };
            let gauge = Gauge::default()
                .label("")
                .gauge_style(Style::new().fg(filled).bg(unfilled))
                .use_unicode(true)
                .ratio(ratio);
            frame.render_widget(gauge, gauge_area);
        }
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    let mut state = State {
        streams: vec![],
        volume_granularity: args.volume_granularity,
        selected: 0,
        longest_filename: 0,
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
