use anyhow::{Context, Result};
use cursive::traits::*;
use cursive::views::{Dialog, ListView, SliderView};
use rodio::Source;

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
    let mut siv = cursive::default();
    siv.add_global_callback('q', |s| s.quit());

    let stream_handle = rodio::OutputStreamBuilder::open_default_stream()?;
    let mixer = stream_handle.mixer();

    // add all the files
    let mut streams = vec![];
    for f in std::fs::read_dir("sounds")? {
        let Ok(f) = f else {
            continue;
        };
        let path = f.path();
        if let Err(e) = add_path(&mut streams, mixer, &path) {
            println!("Could not add {f:?}: {e:?}");
        }
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
