use anyhow::{bail, Context, Result};
use clap::Parser;
use cursive::traits::*;
use cursive::views::{Dialog, ListView, SliderView};
use rodio::Source;
use std::sync::Arc;

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
    sink: Arc<rodio::Sink>,
}

fn add_path(
    streams: &mut Vec<Stream>,
    mixer: &rodio::mixer::Mixer,
    start_volume: f32,
    path: &std::path::Path,
) -> Result<()> {
    let file = std::fs::File::open(path)?;
    let sink = rodio::Sink::connect_new(mixer);
    let decoded = rodio::Decoder::try_from(file)?;
    sink.append(decoded.repeat_infinite());
    sink.set_volume(start_volume);
    if start_volume == 0. {
        sink.pause();
    }
    let filename = path
        .file_stem()
        .context("no filename?")?
        .to_string_lossy()
        .into();
    streams.push(Stream {
        filename,
        sink: Arc::new(sink),
    });
    Ok(())
}

fn add_to_all(s: &mut cursive::Cursive, add: isize) {
    let Some(cbs) = s.call_on_name("sliders_list", |list: &mut ListView| {
        let mut cbs = vec![];
        list.call_on_all(
            &cursive::view::Selector::Name("slider"),
            |slider: &mut SliderView| {
                let val = slider.get_value().saturating_add_signed(add).min(24);
                if let cursive::event::EventResult::Consumed(Some(cb)) = slider.set_value(val) {
                    cbs.push(cb);
                }
            },
        );
        cbs
    }) else {
        return;
    };
    for cb in cbs {
        cb(s);
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    let mut siv = cursive::default();
    siv.add_global_callback('q', |s| s.quit());

    let mut stream_handle = rodio::OutputStreamBuilder::open_default_stream()?;
    stream_handle.log_on_drop(false);
    let mixer = stream_handle.mixer();
    let start_volume = args.start_volume as f32 / args.volume_granularity as f32;

    // add all the files
    let mut streams = vec![];
    for dir in args.dirs {
        let readdir = match std::fs::read_dir(&dir) {
            Ok(readdir) => readdir,
            Err(e) if e.kind() == std::io::ErrorKind::NotADirectory => {
                let path = std::path::Path::new(&dir);
                if let Err(e) = add_path(&mut streams, mixer, start_volume, path) {
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
            if let Err(e) = add_path(&mut streams, mixer, start_volume, &path) {
                println!("Could not add {path:?}: {e:?}");
            }
        }
    }
    if streams.is_empty() {
        bail!("No media found");
    }
    let mut list = ListView::new();
    for stream in &streams {
        let sink = stream.sink.clone();
        list = list.child(
            &stream.filename,
            SliderView::horizontal(args.volume_granularity.into())
                .value(args.start_volume.into())
                .on_change(move |_s, v| {
                    if v == 0 {
                        sink.pause()
                    } else {
                        sink.play()
                    };
                    sink.set_volume(v as f32 / args.volume_granularity as f32);
                })
                .with_name("slider"),
        );
    }
    let dialog = Dialog::new()
        .title("Noises")
        .content(list.with_name("sliders_list"));

    siv.add_global_callback('+', |s| add_to_all(s, 1));
    siv.add_global_callback('-', |s| add_to_all(s, -1));
    let streams_clone: Vec<_> = streams.iter().map(|s| s.sink.clone()).collect();
    let mut paused = false;
    siv.add_global_callback(' ', move |_s| {
        if paused {
            streams_clone.iter().for_each(|stream| {
                if stream.volume() != 0. {
                    stream.play()
                }
            });
            paused = false;
        } else {
            streams_clone.iter().for_each(|stream| stream.pause());
            paused = true;
        }
    });
    siv.add_layer(dialog);
    siv.run();

    Ok(())
}
