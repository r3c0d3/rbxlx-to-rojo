use log::info;
use rbxlx_to_rojo::{filesystem::FileSystem, process_instructions};
use std::{
    fmt, fs,
    io::{self, Write},
    path::PathBuf,
    sync::{Arc, RwLock},
};

#[derive(Debug)]
enum Problem {
    DecodeError(rbx_xml::DecodeError),
    IoError(&'static str, io::Error),
    NFDCancel,
    NFDError(String),
}

impl fmt::Display for Problem {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Problem::DecodeError(error) => write!(
                formatter,
                "While attempting to decode the place file, at {} rbx_xml didn't know what to do",
                error,
            ),

            Problem::IoError(doing_what, error) => {
                write!(formatter, "While attempting to {}, {}", doing_what, error)
            }

            Problem::NFDCancel => write!(formatter, "Didn't choose a file."),

            Problem::NFDError(error) => write!(
                formatter,
                "Something went wrong when choosing a file: {}",
                error,
            ),
        }
    }
}

struct WrappedLogger {
    log: env_logger::Logger,
    log_file: Arc<RwLock<Option<fs::File>>>,
}

impl log::Log for WrappedLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        self.log.enabled(metadata)
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            self.log.log(record);

            if let Some(ref mut log_file) = &mut *self.log_file.write().unwrap() {
                log_file
                    .write(format!("{}\r\n", record.args()).as_bytes())
                    .ok();
            }
        }
    }

    fn flush(&self) {}
}

fn routine() -> Result<(), Problem> {
    let env_logger = env_logger::Builder::new()
        .filter_level(log::LevelFilter::Info)
        .build();

    let log_file = Arc::new(RwLock::new(None));
    let logger = WrappedLogger {
        log: env_logger,
        log_file: Arc::clone(&log_file),
    };

    log::set_boxed_logger(Box::new(logger)).unwrap();
    log::set_max_level(log::LevelFilter::Info);

    info!("rbxlx-to-rojo {}", env!("CARGO_PKG_VERSION"));

    info!("Select a place file.");
    let rbxlx_path = PathBuf::from(match std::env::args().nth(1) {
        Some(text) => text,
        None => match nfd::open_file_dialog(Some("rbxlx,rbxmx"), None)
            .map_err(|error| Problem::NFDError(error.to_string()))?
        {
            nfd::Response::Okay(path) => path,
            nfd::Response::Cancel => Err(Problem::NFDCancel)?,
            _ => unreachable!(),
        },
    });

    info!("Opening place file");
    let rbxlx_source = fs::File::open(&rbxlx_path)
        .map_err(|error| Problem::IoError("read the place file", error))?;
    info!("Decoding place file, this is the longest part...");
    let tree = rbx_xml::from_reader_default(&rbxlx_source).map_err(Problem::DecodeError)?;

    info!("Select the path to put your Rojo project in.");
    let root = PathBuf::from(match std::env::args().nth(2) {
        Some(text) => text,
        None => match nfd::open_pick_folder(Some(&rbxlx_path.parent().unwrap().to_string_lossy()))
            .map_err(|error| Problem::NFDError(error.to_string()))?
        {
            nfd::Response::Okay(path) => path,
            nfd::Response::Cancel => Err(Problem::NFDCancel)?,
            _ => unreachable!(),
        },
    });

    let mut filesystem = FileSystem::from_root(root.join(rbxlx_path.file_stem().unwrap()).into());

    log_file.write().unwrap().replace(
        fs::File::create(root.join("rbxlx-to-rojo.log"))
            .map_err(|error| Problem::IoError("couldn't create log file", error))?,
    );

    info!("Starting processing, please wait a bit...");
    process_instructions(&tree, &mut filesystem);
    info!("Done! Check rbxlx-to-rojo.log for a full log.");
    Ok(())
}

fn main() {
    if let Err(error) = routine() {
        eprintln!("An error occurred while using rbxlx-to-rojo.");
        eprintln!("{}", error);
    }
}
