use instant::{Duration, Instant, SystemTime};
use std::{fs::File, io::Write, path::Path};

#[macro_export]
macro_rules! log_to {
    ($target:expr, $($arg:tt)*) => {
        $target.log(format!($($arg)*))
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LogType {
    General,
    Network,
    AudioPerformance,
}

#[derive(Clone)]
pub struct LogEndpoint {
    pub ty: LogType,
    tx: flume::Sender<String>,
}

impl LogEndpoint {
    pub fn log(&self, s: String) {
        self.tx.send(s).unwrap_or_else(|e| {
            panic!(
                "Log endpoint for '{:?}' has no receivers. Error: `{:?}`",
                self.ty, e
            )
        });
    }
}

pub type FmtF = Box<dyn Fn(String, &LogEndpoint) -> String>;

pub struct LogEndpointBuilder<'a> {
    ty: LogType,
    logger: &'a mut Logger,

    fmt: Option<FmtF>,
    file: Option<File>,
    print: bool,
}

impl<'a> LogEndpointBuilder<'a> {
    pub fn fmt(mut self, f: impl Fn(String, &LogEndpoint) -> String + 'static) -> Self {
        self.fmt = Some(Box::new(f) as Box<_>);
        self
    }

    #[allow(unused_mut, unused_variables)]
    pub fn path(mut self, path: impl AsRef<Path>) -> Self {
        // Ignore file if we are running on web.
        #[cfg(not(target_family = "wasm"))]
        {
            use std::fs::OpenOptions;
            self.file = Some(
                OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .read(true)
                    .open(path)
                    .unwrap(),
            );
        }
        self
    }

    pub fn print(mut self, print: bool) -> Self {
        self.print = print;
        self
    }

    pub fn build(self) -> LogEndpoint {
        let (tx, rx) = flume::unbounded();
        let endpoint = LogEndpoint { ty: self.ty, tx };
        self.logger.types.push(LogEndpointEntry {
            rx,
            fmt: self.fmt.unwrap_or_else(|| {
                Box::new(|s, endpoint| {
                    format!(
                        "/{}/ [{:?}] {}",
                        SystemTime::now()
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                        endpoint.ty,
                        s
                    )
                })
            }),
            endpoint: endpoint.clone(),
            file: self.file,
            print: self.print,
        });
        endpoint
    }
}

struct LogEndpointEntry {
    rx: flume::Receiver<String>,
    fmt: FmtF,
    endpoint: LogEndpoint,
    file: Option<File>,
    print: bool,
}

pub struct Logger {
    types: Vec<LogEndpointEntry>,
    flush_interval: Duration,
    last_flush: Instant,
}

impl Logger {
    pub fn new(flush_interval: Duration) -> Self {
        Logger {
            types: Vec::new(),
            flush_interval,
            last_flush: Instant::now(),
        }
    }

    pub fn init_endpoint(&mut self, ty: LogType) -> LogEndpointBuilder {
        LogEndpointBuilder {
            ty,
            logger: self,
            fmt: None,
            file: None,
            print: true,
        }
    }

    pub fn flush(&mut self) {
        let now = Instant::now();
        let should_flush = now.duration_since(self.last_flush) >= self.flush_interval;
        if should_flush {
            self.last_flush = now;
        }

        for entry in &mut self.types {
            for info in entry.rx.drain() {
                let line = (entry.fmt)(info, &entry.endpoint);
                if let Some(file) = &mut entry.file {
                    file.write_all(line.as_bytes()).unwrap();
                    file.write_all(b"\n").unwrap();
                }

                if entry.print {
                    println!("{}", line);
                }
            }

            if should_flush {
                if let Some(file) = &mut entry.file {
                    file.flush().unwrap();
                }
            }
        }
    }
}
