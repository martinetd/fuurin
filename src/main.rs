use anyhow::Result;
use rodio::Source;

fn main() -> Result<()> {
    /*
    let stream_handle = rodio::OutputStreamBuilder::open_default_stream()?;
    let sink = rodio::Sink::connect_new(stream_handle.mixer());

    let file = std::fs::File::open("sounds/city.ogg")?;
    sink.append(rodio::Decoder::try_from(file)?);
    let file = std::fs::File::open("sounds/rain.ogg")?;
    sink.append(rodio::Decoder::try_from(file)?);

    sink.sleep_until_end();
    */
    //    let (controller, mixer) = rodio::mixer::mixer(2, 44_100);
    let stream_handle = rodio::OutputStreamBuilder::open_default_stream()?;
    let mixer = stream_handle.mixer();
    //  let sink = rodio::Sink::connect_new(stream_handle.mixer());

    //sink.append(mixer);
    println!("1");
    let file = std::fs::File::open("sounds/rain.ogg")?;
    let sink = rodio::Sink::connect_new(mixer);
    sink.append(rodio::Decoder::try_from(file)?.repeat_infinite());
    sink.play();

    let source = rodio::source::SineWave::new(440.0)
        .take_duration(std::time::Duration::from_secs_f32(0.25))
        .amplify(0.20);
    mixer.add(source);

    let file = std::fs::File::open("sounds/city.ogg")?;
    mixer.add(rodio::Decoder::try_from(file)?);
    println!("1");
    println!("2");
    std::thread::sleep(std::time::Duration::from_secs(10));
    //    mixer.sleep_until_end();
    println!("3");
    Ok(())
}
