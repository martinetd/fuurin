use anyhow::{bail, Context, Result};
use clap::Parser;
use cursive::traits::*;
use cursive::views::{Dialog, ListView, SliderView};
use rodio::Source;

#[derive(Parser)]
#[command(version, about, long_about = Some("Mix up sounds from directories"))]
struct Args {
    #[arg(default_values_t = vec!["sounds".to_string()])]
    dirs: Vec<String>,
}

struct Stream {
    filename: String,
    sink: rodio::Sink,
}

fn add_path(
    streams: &mut Vec<Stream>,
    mixer: &rodio::mixer::Mixer,
    path: &std::path::Path,
) -> Result<()> {
    let file = std::fs::File::open(path)?;
    let sink = rodio::Sink::connect_new(mixer);
    let decoded = rodio::Decoder::try_from(file)?;
    sink.append(decoded.repeat_infinite());
    sink.set_volume(0.);
    sink.pause();
    let filename = path
        .file_stem()
        .context("no filename?")?
        .to_string_lossy()
        .into();
    streams.push(Stream { filename, sink });
    Ok(())
}

fn main() -> Result<()> {
    let args = Args::parse();
    let mut siv = cursive::default();
    siv.add_global_callback('q', |s| s.quit());

    let mut stream_handle = rodio::OutputStreamBuilder::open_default_stream()?;
    stream_handle.log_on_drop(false);
    let mixer = stream_handle.mixer();

    // add all the files
    let mut streams = vec![];
    for dir in args.dirs {
        let readdir = match std::fs::read_dir(&dir) {
            Ok(readdir) => readdir,
            Err(e) if e.kind() == std::io::ErrorKind::NotADirectory => {
                let path = std::path::Path::new(&dir);
                if let Err(e) = add_path(&mut streams, mixer, path) {
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
            if let Err(e) = add_path(&mut streams, mixer, &path) {
                println!("Could not add {path:?}: {e:?}");
            }
        }
    }
    if streams.is_empty() {
        bail!("No media found");
    }
    let mut list = ListView::new();
    for stream in streams {
        list = list.child(
            &stream.filename,
            SliderView::horizontal(10).value(0).on_change(move |_s, v| {
                if v == 0 {
                    stream.sink.pause()
                } else {
                    stream.sink.play()
                };
                stream.sink.set_volume(v as f32 / 10.);
            }),
        );
    }
    siv.add_layer(
        Dialog::new()
            .title("Noises")
            .content(list)
            .with_name("main"),
    );
    siv.run();

    Ok(())
}
