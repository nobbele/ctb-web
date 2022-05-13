use std::{ffi::OsStr, path::PathBuf, process::Command};

fn main() {
    let song_path = PathBuf::from(std::env::args().nth(1).expect("Song path"));
    let files = std::fs::read_dir(&song_path)
        .unwrap()
        .map(|res| res.map(|e| e.path()))
        .collect::<Result<Vec<_>, std::io::Error>>()
        .unwrap();

    let diffs = files
        .iter()
        .filter(|f| f.extension() == Some(OsStr::new("osu")))
        .collect::<Vec<_>>();

    let base_diff_path = *diffs.first().unwrap();
    let base_diff = osu_parser::load_content(
        &std::fs::read_to_string(base_diff_path).unwrap(),
        osu_parser::BeatmapParseOptions::default(),
    )
    .unwrap();

    let audio_file_path = &base_diff.info.general_data.audio_file_name;
    let bg_file_path = match &base_diff.events.first().unwrap() {
        osu_types::Event::Background { filename, .. } => filename,
        _ => panic!(),
    };
    let set_name = base_diff.info.metadata.title;

    let target_dir = PathBuf::from("resources").join(set_name);
    std::fs::create_dir(&target_dir).unwrap();

    Command::new("ffmpeg")
        .args([
            "-i",
            song_path.join(audio_file_path).to_str().unwrap(),
            target_dir.join("audio.wav").to_str().unwrap(),
        ])
        .output()
        .unwrap();

    Command::new("convert")
        .args(dbg!([
            song_path.join(bg_file_path).to_str().unwrap(),
            target_dir.join("bg.png").to_str().unwrap(),
        ]))
        .output()
        .unwrap();

    for diff in diffs {
        let beatmap = osu_parser::load_content(
            &std::fs::read_to_string(diff).unwrap(),
            osu_parser::BeatmapParseOptions::default(),
        )
        .unwrap();
        std::fs::copy(
            diff,
            target_dir.join(format!("{}.osu", beatmap.info.metadata.version)),
        )
        .unwrap();
    }
}
