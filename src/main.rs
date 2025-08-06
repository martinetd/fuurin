use anyhow::Result;
use rodio::Source;

struct Stream {
    filename: String,
    sink: rodio::Sink,
}

fn main() -> Result<()> {
    let stream_handle = rodio::OutputStreamBuilder::open_default_stream()?;
    let mixer = stream_handle.mixer();

    // add all the files
    let mut streams = vec![];
    for f in std::fs::read_dir("sounds")? {
        let path = f?.path();
        let file = std::fs::File::open(&path)?;
        let sink = rodio::Sink::connect_new(mixer);
        let decoded = match rodio::Decoder::try_from(file) {
            Ok(d) => d,
            Err(e) => {
                println!("Could not read {path:?}: {e:?}");
                continue;
            }
        };
        sink.append(decoded.repeat_infinite());
        sink.set_volume(0.5);
        streams.push(Stream {
            filename: "rain".to_string(),
            sink,
        });
    }

    let source = rodio::source::SineWave::new(440.0)
        .take_duration(std::time::Duration::from_secs_f32(0.25))
        .amplify(0.20);
    mixer.add(source);

    std::thread::sleep(std::time::Duration::from_secs(10));
    Ok(())
}
