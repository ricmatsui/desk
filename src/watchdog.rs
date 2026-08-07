use std::time::{Duration, Instant};

pub fn spawn() {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut last_tick = Instant::now();

        loop {
            interval.tick().await;

            let now = Instant::now();
            let elapsed = now.duration_since(last_tick);
            last_tick = now;

            let proc_status = read_proc_self_status().await;

            tracing::info!(
                "watchdog elapsed={}s {}",
                elapsed.as_secs(),
                proc_status,
            );
        }
    });
}

async fn read_proc_self_status() -> String {
    #[cfg(target_os = "linux")]
    {
        match tokio::fs::read_to_string("/proc/self/status").await {
            Ok(contents) => {
                let mut fields = Vec::new();
                for line in contents.lines() {
                    if let Some((key, value)) = line.split_once(':')
                        && matches!(
                            key,
                            "VmRSS"
                                | "VmSwap"
                                | "voluntary_ctxt_switches"
                                | "nonvoluntary_ctxt_switches"
                        )
                    {
                        fields.push(format!("{}={}", key, value.trim()));
                    }
                }
                fields.join(" ")
            }
            Err(e) => format!("status_error={}", e),
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        String::new()
    }
}
