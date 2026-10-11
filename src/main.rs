use evdev::InputEventKind;
use rand::seq::SliceRandom;
use rand::Rng;
use rodio::buffer::SamplesBuffer;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Source};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::mpsc;

#[derive(Deserialize, Debug)]
struct Config {
    defaults: Vec<String>,
    mappings: HashMap<String, String>,
}

#[derive(Clone)]
struct DecodedSound {
    samples: Arc<Vec<i16>>,
    channels: u16,
    sample_rate: u32,
}

enum AudioCmd {
    Play(Arc<DecodedSound>),
    Reconnect,
    SetVolume(f32),
    SetHumanize(bool),
}

fn load_volume_config(config_dir: &Path) -> f32 {
    let vol_path = config_dir.join("volume.txt");
    if let Ok(content) = fs::read_to_string(&vol_path) {
        if let Ok(v) = content.trim().parse::<f32>() {
            return if v > 1.0 { (v / 100.0).clamp(0.0, 2.0) } else { v.clamp(0.0, 2.0) };
        }
    }
    if let Ok(env_v) = env::var("CLICKCLICK_VOLUME") {
        if let Ok(v) = env_v.trim().parse::<f32>() {
            return if v > 1.0 { (v / 100.0).clamp(0.0, 2.0) } else { v.clamp(0.0, 2.0) };
        }
    }
    0.75
}

fn load_humanize_config(config_dir: &Path) -> bool {
    let h_path = config_dir.join("humanize.txt");
    if let Ok(content) = fs::read_to_string(&h_path) {
        let val = content.trim().to_lowercase();
        return val == "true" || val == "1" || val == "on";
    }
    if let Ok(env_h) = env::var("CLICKCLICK_HUMANIZE") {
        let val = env_h.trim().to_lowercase();
        return val == "true" || val == "1" || val == "on";
    }
    true // default to organic subtle variation
}

fn load_mice_config(config_dir: &Path) -> bool {
    let m_path = config_dir.join("enable_mice.txt");
    if let Ok(content) = fs::read_to_string(&m_path) {
        let val = content.trim().to_lowercase();
        return val == "true" || val == "1" || val == "on";
    }
    if let Ok(env_m) = env::var("ENABLE_MICE") {
        let val = env_m.trim().to_lowercase();
        return val == "true" || val == "1" || val == "on";
    }
    true
}

fn start_audio_engine(
    initial_volume: f32,
    initial_humanize: bool,
) -> mpsc::UnboundedSender<AudioCmd> {
    let (tx, mut rx) = mpsc::unbounded_channel::<AudioCmd>();

    std::thread::spawn(move || {
        let try_connect = || -> (Option<OutputStream>, Option<OutputStreamHandle>) {
            match OutputStream::try_default() {
                Ok((s, h)) => (Some(s), Some(h)),
                Err(e) => {
                    eprintln!("\x1b[1;33m[WARN]\x1b[0m Audio output open error: {:?}", e);
                    (None, None)
                }
            }
        };

        let (_s, _h) = try_connect();
        let mut _stream: Option<OutputStream> = _s;
        let mut stream_handle: Option<OutputStreamHandle> = _h;
        let mut volume = initial_volume;
        let mut humanize = initial_humanize;

        while let Some(cmd) = rx.blocking_recv() {
            match cmd {
                AudioCmd::Play(sound) => {
                    if stream_handle.is_none() {
                        let (s, h) = try_connect();
                        _stream = s;
                        stream_handle = h;
                    }

                    let mut rng = rand::thread_rng();
                    let speed = if humanize {
                        rng.gen_range(0.975..1.025)
                    } else {
                        1.0
                    };
                    let vol_jitter = if humanize {
                        rng.gen_range(0.95..1.05)
                    } else {
                        1.0
                    };
                    let eff_vol = (volume * vol_jitter).clamp(0.0, 2.0);

                    let source = SamplesBuffer::new(
                        sound.channels,
                        sound.sample_rate,
                        sound.samples.as_ref().clone(),
                    )
                    .speed(speed)
                    .amplify(eff_vol);

                    if let Some(ref handle) = stream_handle {
                        if let Err(e) = handle.play_raw(source.convert_samples()) {
                            eprintln!("\x1b[1;33m[WARN]\x1b[0m Play error: {:?}. Reconnecting audio stream...", e);
                            _stream = None;
                            stream_handle = None;
                            let (s, h) = try_connect();
                            _stream = s;
                            stream_handle = h;
                            if let Some(ref handle2) = stream_handle {
                                let source2 = SamplesBuffer::new(
                                    sound.channels,
                                    sound.sample_rate,
                                    sound.samples.as_ref().clone(),
                                )
                                .speed(speed)
                                .amplify(eff_vol);
                                let _ = handle2.play_raw(source2.convert_samples());
                            }
                        }
                    }
                }
                AudioCmd::Reconnect => {
                    println!("\x1b[1;34m[AUDIO]\x1b[0m Reconnecting audio output stream (SIGUSR1 / Sink Change)...");
                    _stream = None;
                    stream_handle = None;
                    let (s, h) = try_connect();
                    _stream = s;
                    stream_handle = h;
                    if stream_handle.is_some() {
                        println!("\x1b[1;32m[+]\x1b[0m Audio output reconnected successfully.");
                    }
                }
                AudioCmd::SetVolume(v) => {
                    println!("\x1b[1;36m[AUDIO]\x1b[0m Volume updated: {:.2}", v);
                    volume = v;
                }
                AudioCmd::SetHumanize(h) => {
                    println!("\x1b[1;36m[AUDIO]\x1b[0m Humanize mode: {}", h);
                    humanize = h;
                }
            }
        }
    });

    tx
}

fn print_help() {
    println!(
        "\x1b[1;32mClickClick Daemon (clickclickd)\x1b[0m v{} — Low-latency mechanical acoustic feedback daemon for Linux
Usage:
  clickclickd [OPTIONS] [ASSET_DIR]

Arguments:
  [ASSET_DIR]    Path to switch preset directory containing config.json and audio files
                 (Default: ~/.config/clickclick/active or system/repo presets/nk_cream)

Options:
  -h, --help     Print this help message
  -V, --version  Print version information

Signals:
  SIGUSR1        Reconnect audio output stream (e.g. after PipeWire default sink switch)
  SIGUSR2        Hot-reload volume, humanization, and mouse click settings without restarting",
        env!("CARGO_PKG_VERSION")
    );
}

fn resolve_default_asset_dir() -> Option<PathBuf> {
    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let mut candidates = vec![
        PathBuf::from(&home).join(".config/clickclick/active"),
        PathBuf::from("presets/nk_cream"),
    ];
    if let Ok(exe) = env::current_exe() {
        if let Some(bin_dir) = exe.parent() {
            candidates.push(bin_dir.join("../share/clickclick/presets/nk_cream"));
            candidates.push(bin_dir.join("../presets/nk_cream"));
        }
    }
    candidates.push(PathBuf::from("/usr/local/share/clickclick/presets/nk_cream"));
    candidates.push(PathBuf::from("/usr/share/clickclick/presets/nk_cream"));

    candidates.into_iter().find(|p| p.join("config.json").exists())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Elevate priority if permissions allow
    unsafe {
        libc::nice(-10);
    }

    let args: Vec<String> = env::args().collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        print_help();
        return Ok(());
    }
    if args.iter().any(|a| a == "-V" || a == "--version") {
        println!("clickclickd {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let asset_dir: PathBuf = if args.len() > 1 && !args[1].starts_with('-') {
        PathBuf::from(&args[1])
    } else if let Some(found) = resolve_default_asset_dir() {
        found
    } else {
        eprintln!("\x1b[1;31m[ERROR]\x1b[0m No sound preset found! Provide a path: clickclickd <asset_dir>");
        std::process::exit(1);
    };

    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let config_dir: PathBuf = {
        let default_cfg = PathBuf::from(&home).join(".config/clickclick");
        if default_cfg.exists() || fs::create_dir_all(&default_cfg).is_ok() {
            default_cfg
        } else {
            asset_dir
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| asset_dir.clone())
        }
    };

    let pid_file = config_dir.join("daemon.pid");
    let _ = fs::write(&pid_file, format!("{}\n", std::process::id()));

    let enable_trackpads = env::var("ENABLE_TRACKPADS")
        .unwrap_or_else(|_| "false".to_string())
        .to_lowercase() == "true";

    let raw_identifiers = env::var("WC_TRACKPAD_IDS").unwrap_or_else(|_| "touchpad,trackpad".to_string());
    let trackpad_keywords: Vec<String> = raw_identifiers
        .split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect();

    let initial_volume = load_volume_config(&config_dir);
    let initial_humanize = load_humanize_config(&config_dir);
    let initial_mice = load_mice_config(&config_dir);
    let enable_mice = Arc::new(AtomicBool::new(initial_mice));

    println!(
        "\x1b[1;34m[INIT]\x1b[0m Volume: {:.2}, Humanize: {}, Mouse Clicks: {}",
        initial_volume, initial_humanize, initial_mice
    );

    let audio_tx = start_audio_engine(initial_volume, initial_humanize);

    // Watch for SIGUSR1 (stream reconnect)
    let audio_tx_sig1 = audio_tx.clone();
    tokio::spawn(async move {
        if let Ok(mut sig) = signal(SignalKind::user_defined1()) {
            while sig.recv().await.is_some() {
                let _ = audio_tx_sig1.send(AudioCmd::Reconnect);
            }
        }
    });

    // Watch for SIGUSR2 (config reload: volume/humanize/mice)
    let audio_tx_sig2 = audio_tx.clone();
    let config_dir_sig = config_dir.clone();
    let enable_mice_sig = enable_mice.clone();
    tokio::spawn(async move {
        if let Ok(mut sig) = signal(SignalKind::user_defined2()) {
            while sig.recv().await.is_some() {
                let v = load_volume_config(&config_dir_sig);
                let h = load_humanize_config(&config_dir_sig);
                let m = load_mice_config(&config_dir_sig);
                enable_mice_sig.store(m, Ordering::Relaxed);
                println!("\x1b[1;36m[AUDIO]\x1b[0m Mouse clicks mode: {}", m);
                let _ = audio_tx_sig2.send(AudioCmd::SetVolume(v));
                let _ = audio_tx_sig2.send(AudioCmd::SetHumanize(h));
            }
        }
    });

    let config_path = asset_dir.join("config.json");
    if !config_path.exists() {
        eprintln!("\x1b[1;31m[ERROR]\x1b[0m Missing config.json in: {:?}", asset_dir);
        std::process::exit(1);
    }
    let config_str = fs::read_to_string(&config_path)?;
    let config: Config = serde_json::from_str(&config_str)?;

    let mouse_sound: Option<Arc<DecodedSound>> = {
        let mouse_p1 = asset_dir.join("mouse.wav");
        let mouse_p2 = asset_dir.parent().unwrap_or_else(|| &asset_dir).join("mouse.wav");
        let target_path = if mouse_p1.exists() {
            Some(mouse_p1)
        } else if mouse_p2.exists() {
            Some(mouse_p2)
        } else {
            None
        };

        if let Some(p) = target_path {
            match fs::read(&p) {
                Ok(bytes) => match Decoder::new(Cursor::new(bytes)) {
                    Ok(decoder) => {
                        let channels = decoder.channels();
                        let sample_rate = decoder.sample_rate();
                        let samples: Vec<i16> = decoder.collect();
                        println!("\x1b[1;32m[+]\x1b[0m Soft mouse click sound loaded: {:?}", p);
                        Some(Arc::new(DecodedSound {
                            samples: Arc::new(samples),
                            channels,
                            sample_rate,
                        }))
                    }
                    Err(e) => {
                        eprintln!("\x1b[1;33m[WARN]\x1b[0m Failed to decode mouse.wav: {:?}", e);
                        None
                    }
                },
                Err(e) => {
                    eprintln!("\x1b[1;33m[WARN]\x1b[0m Failed to read mouse.wav: {:?}", e);
                    None
                }
            }
        } else {
            None
        }
    };
    let mouse_sound_arc = Arc::new(mouse_sound);

    let mut required_sounds = HashSet::new();
    for v in &config.defaults {
        required_sounds.insert(v.clone());
    }
    for v in config.mappings.values() {
        required_sounds.insert(v.clone());
    }

    let mut sounds: HashMap<String, Arc<DecodedSound>> = HashMap::new();
    for filename in required_sounds {
        let path = asset_dir.join(&filename);
        match fs::read(&path) {
            Ok(bytes) => {
                let cursor = Cursor::new(bytes);
                match Decoder::new(cursor) {
                    Ok(decoder) => {
                        let channels = decoder.channels();
                        let sample_rate = decoder.sample_rate();
                        let samples: Vec<i16> = decoder.collect();
                        sounds.insert(filename, Arc::new(DecodedSound {
                            samples: Arc::new(samples),
                            channels,
                            sample_rate,
                        }));
                    }
                    Err(e) => {
                        eprintln!("\x1b[1;33m[WARN]\x1b[0m Failed to decode wav: {}. Error: {:?}", filename, e);
                    }
                }
            }
            Err(e) => {
                eprintln!("\x1b[1;33m[WARN]\x1b[0m Failed to load wav: {}. Error: {:?}", filename, e);
            }
        }
    }

    if sounds.is_empty() {
        eprintln!("\x1b[1;31m[ERROR]\x1b[0m No sounds loaded! Check your config.json and .wav files.");
        std::process::exit(1);
    }

    let mut sound_cache: Vec<Option<Arc<DecodedSound>>> = vec![None; 65536];
    for (code_str, filename) in config.mappings.iter() {
        if let Ok(code) = code_str.parse::<usize>() {
            if code < 65536 {
                if let Some(s) = sounds.get(filename) {
                    sound_cache[code] = Some(s.clone());
                }
            }
        }
    }

    let default_sound_objs: Vec<Arc<DecodedSound>> = config
        .defaults
        .iter()
        .filter_map(|f| sounds.get(f).cloned())
        .collect();

    let cache_arc = Arc::new(sound_cache);
    let defaults_arc = Arc::new(default_sound_objs);

    println!("\x1b[1;34m[CORE]\x1b[0m Engine started. Monitoring devices...");

    let mut monitored_devices = HashSet::new();
    let (tx_dead, mut rx_dead) = mpsc::unbounded_channel::<String>();

    loop {
        // Cleanup dead devices
        while let Ok(dead_path) = rx_dead.try_recv() {
            monitored_devices.remove(&dead_path);
        }

        // Device discovery loop
        let devices = evdev::enumerate();
        for (path, device) in devices {
            let path_str = path.to_string_lossy().to_string();
            if monitored_devices.contains(&path_str) {
                continue;
            }
            
            let name = device.name().unwrap_or("").to_lowercase();
            if !enable_trackpads && trackpad_keywords.iter().any(|k| name.contains(k)) {
                continue;
            }

            // Check if device supports EV_KEY events (Keyboards/Mice)
            if device.supported_keys().is_some() {
                println!("\x1b[1;32m[+]\x1b[0m Connected: {}", name);
                monitored_devices.insert(path_str.clone());

                let cache = cache_arc.clone();
                let defaults = defaults_arc.clone();
                let mouse_sound = mouse_sound_arc.clone();
                let mice_enabled = enable_mice.clone();
                let audio_tx = audio_tx.clone();
                let tx = tx_dead.clone();
                
                if let Ok(mut event_stream) = device.into_event_stream() {
                    tokio::spawn(async move {
                        while let Ok(event) = event_stream.next_event().await {
                            if event.value() == 1 { // Key Down
                                if let InputEventKind::Key(key) = event.kind() {
                                    let code = key.0 as usize;
                                    if (272..=319).contains(&code) {
                                        if mice_enabled.load(Ordering::Relaxed) {
                                            if let Some(ref m_sound) = *mouse_sound {
                                                let _ = audio_tx.send(AudioCmd::Play(m_sound.clone()));
                                            }
                                        }
                                        continue;
                                    }
                                    let sound_opt = if code < 65536 { cache[code].clone() } else { None };
                                    
                                    let sound = sound_opt.or_else(|| {
                                        if !defaults.is_empty() {
                                            let mut rng = rand::thread_rng();
                                            defaults.choose(&mut rng).cloned()
                                        } else {
                                            None
                                        }
                                    });
                                    
                                    if let Some(s) = sound {
                                        let _ = audio_tx.send(AudioCmd::Play(s));
                                    }
                                }
                            }
                        }
                        // If loop breaks, device disconnected
                        println!("\x1b[1;33m[-]\x1b[0m Disconnected: {}", path_str);
                        let _ = tx.send(path_str);
                    });
                }
            }
        }
        
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
