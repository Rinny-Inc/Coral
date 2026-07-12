use coral_protocol::packets::play::chat::builder::{ChatAppender, ChatBuilder, ChatColor};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};
use tokio::sync::RwLock;

use crate::{Command, CommandResult, make_handler};

#[derive(Clone, Copy)]
pub struct Sample {
    pub timestamp: Instant,
    pub cpu_percent: f32,
    pub mem_bytes: u64,
}

pub struct ResourceMonitor {
    samples: RwLock<VecDeque<Sample>>,
    system: RwLock<System>,
    pid: Pid,
    start: Instant,
}

const MAX_SAMPLES: usize = 900; // 15 min at 1/sec

impl ResourceMonitor {
    pub fn new() -> Self {
        let pid = sysinfo::get_current_pid().expect("failed to get current pid");
        let system = System::new_with_specifics(
            RefreshKind::nothing()
                .with_processes(ProcessRefreshKind::nothing().with_cpu().with_memory()),
        );
        Self {
            samples: RwLock::new(VecDeque::with_capacity(MAX_SAMPLES)),
            system: RwLock::new(system),
            pid,
            start: Instant::now(),
        }
    }

    pub async fn sample(&self) {
        let (cpu_percent, mem_bytes) = {
            let mut sys = self.system.write().await;
            sys.refresh_processes_specifics(
                ProcessesToUpdate::Some(&[self.pid]),
                true,
                ProcessRefreshKind::nothing().with_cpu().with_memory(),
            );
            match sys.process(self.pid) {
                Some(proc) => (proc.cpu_usage(), proc.memory()),
                None => (0.0, 0),
            }
        };

        let mut samples = self.samples.write().await;
        if samples.len() >= MAX_SAMPLES {
            samples.pop_front();
        }
        samples.push_back(Sample {
            timestamp: Instant::now(),
            cpu_percent,
            mem_bytes,
        });
    }

    /// Returns the average, plus whether the window has full coverage.
    /// None = no data at all. Some((.., false)) = partial (server younger than window).
    pub async fn average(&self, window: Duration) -> Option<(f32, u64, u64, bool)> {
        let samples = self.samples.read().await;
        if samples.is_empty() {
            return None;
        }

        let now = Instant::now();
        let cutoff = now.checked_sub(window).unwrap_or(now);
        let relevant: Vec<&Sample> = samples.iter().filter(|s| s.timestamp >= cutoff).collect();
        if relevant.is_empty() {
            return None;
        }

        let avg_cpu = relevant.iter().map(|s| s.cpu_percent).sum::<f32>() / relevant.len() as f32;
        let avg_mem = relevant.iter().map(|s| s.mem_bytes).sum::<u64>() / relevant.len() as u64;
        let peak_mem = relevant.iter().map(|s| s.mem_bytes).max().unwrap_or(0);

        // has the server been running at least as long as this window?
        let full_coverage = now.duration_since(self.start) >= window;

        Some((avg_cpu, avg_mem, peak_mem, full_coverage))
    }
}

pub fn command(monitor: Arc<ResourceMonitor>) -> Command {
    Command {
        name: "usage",
        aliases: vec!["mem", "cpu", "perf"],
        description: "Show CPU and memory usage over time",
        usage: "/usage",
        handler: make_handler(move |_ctx| {
            let monitor = monitor.clone();
            async move {
                let windows = [
                    ("1s", Duration::from_secs(1)),
                    ("30s", Duration::from_secs(30)),
                    ("1m", Duration::from_secs(60)),
                    ("5m", Duration::from_secs(300)),
                    ("10m", Duration::from_secs(600)),
                    ("15m", Duration::from_secs(900)),
                ];

                let mut appender = ChatAppender::new();
                appender = appender.add(
                    ChatBuilder::new("Resource Usage\n")
                        .color(ChatColor::Gold)
                        .bold(),
                );

                for (label, window) in windows {
                    match monitor.average(window).await {
                        None => {
                            appender = appender
                                .add(
                                    ChatBuilder::new(format!("{:>4}  ", label))
                                        .color(ChatColor::Yellow),
                                )
                                .add(
                                    ChatBuilder::new("no data yet\n")
                                        .color(ChatColor::DarkGray)
                                        .italic(),
                                );
                        }
                        Some((cpu, avg_mem, peak_mem, full)) => {
                            appender = appender
                                .add(
                                    ChatBuilder::new(format!("{:>4}  ", label))
                                        .color(ChatColor::Yellow),
                                )
                                .add(
                                    ChatBuilder::new(format!("CPU {:>5.1}%  ", cpu))
                                        .color(ChatColor::White),
                                )
                                .add(
                                    ChatBuilder::new(format!("MEM {}  ", fmt_bytes(avg_mem)))
                                        .color(ChatColor::White),
                                )
                                .add(
                                    ChatBuilder::new(format!("(peak {})", fmt_bytes(peak_mem)))
                                        .color(ChatColor::Gray),
                                );

                            // mark windows that don't have full history yet
                            if !full {
                                appender = appender.add(
                                    ChatBuilder::new(" *")
                                        .color(ChatColor::DarkGray)
                                        .hover_text(
                                            "Partial — server hasn't been running this long yet",
                                        ),
                                );
                            }
                            appender = appender.add(ChatBuilder::new("\n").color(ChatColor::White));
                        }
                    }
                }

                CommandResult::Success(appender.build())
            }
        }),
    }
}

fn fmt_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else {
        format!("{:.0} KB", bytes as f64 / KB as f64)
    }
}
