//! Read-only, process-free host sampler shared by Apple native and Agent hosts.
use containerhub_core::*;
use std::{path::Path, sync::Mutex, time::Instant};
use sysinfo::{Disks, Networks, System};
#[derive(Default)]
pub struct HostSampler(Mutex<Option<Sampler>>);
struct Sampler {
    system: System,
    disks: Disks,
    networks: Networks,
    last: Instant,
    cached: Option<HostSnapshot>,
}

impl HostSampler {
    pub fn sample(&self, environment: &str) -> Result<HostSnapshot, ManagementError> {
        let mut guard = self
            .0
            .lock()
            .map_err(|_| ManagementError::new(ErrorKind::Unavailable, "主机采样不可用"))?;
        let s = guard.get_or_insert_with(|| Sampler {
            system: System::new(),
            disks: Disks::new(),
            networks: Networks::new(),
            last: Instant::now(),
            cached: None,
        });
        let elapsed = s.last.elapsed();
        if elapsed < sysinfo::MINIMUM_CPU_UPDATE_INTERVAL
            && let Some(cached) = &s.cached
        {
            let mut cached = cached.clone();
            cached.environment = environment.into();
            cached.validate()?;
            return Ok(cached);
        }
        let ready = s.cached.is_some();
        s.system.refresh_cpu_usage();
        s.system.refresh_memory();
        s.networks.refresh(true);
        s.disks.refresh(true);
        let seconds = elapsed.as_secs_f64().max(0.001);
        let interfaces: Vec<_> = s
            .networks
            .iter()
            .filter(|(name, _)| !matches!(name.as_str(), "lo" | "lo0"))
            .map(|(_, v)| v)
            .collect();
        let receive = interfaces
            .iter()
            .fold(0u64, |sum, v| sum.saturating_add(v.received()));
        let send = interfaces
            .iter()
            .fold(0u64, |sum, v| sum.saturating_add(v.transmitted()));
        let disk = s
            .disks
            .list()
            .iter()
            .find(|d| d.mount_point() == Path::new("/System/Volumes/Data"))
            .or_else(|| {
                s.disks
                    .list()
                    .iter()
                    .find(|d| d.mount_point() == Path::new("/"))
            });
        let rate = |bytes: u64| ready.then_some(bytes as f64 / seconds);
        let total = s.system.total_memory();
        let sample = HostSnapshot {
            environment: environment.into(),
            observed_at: now(),
            hostname: System::host_name(),
            os: System::name().map(|n| format!("{n} {}", System::os_version().unwrap_or_default())),
            architecture: System::cpu_arch(),
            processor: s.system.cpus().first().map(|c| c.brand().to_owned()),
            logical_cpus: s.system.cpus().len() as u32,
            uptime_seconds: System::uptime(),
            cpu_percent: (ready && !s.system.cpus().is_empty())
                .then(|| (s.system.global_cpu_usage() as f64).clamp(0., 100.)),
            memory_used_bytes: (total > 0).then(|| s.system.used_memory()),
            memory_total_bytes: (total > 0).then_some(total),
            disk_path: disk.map(|d| d.mount_point().display().to_string()),
            disk_used_bytes: disk.map(|d| d.total_space().saturating_sub(d.available_space())),
            disk_total_bytes: disk.map(|d| d.total_space()),
            disk_read_bytes_per_second: disk.and_then(|d| rate(d.usage().read_bytes)),
            disk_write_bytes_per_second: disk.and_then(|d| rate(d.usage().written_bytes)),
            network_receive_bytes_per_second: (!interfaces.is_empty())
                .then(|| rate(receive))
                .flatten(),
            network_send_bytes_per_second: (!interfaces.is_empty()).then(|| rate(send)).flatten(),
            sample: false,
        };
        sample.validate()?;
        s.last = Instant::now();
        s.cached = Some(sample.clone());
        Ok(sample)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn first_sample_has_no_invented_rates_and_cache_preserves_requested_identity() {
        let sampler = HostSampler::default();
        let first = sampler.sample("host-a").unwrap();
        assert_eq!(first.cpu_percent, None);
        assert_eq!(first.network_receive_bytes_per_second, None);
        assert!(!first.sample);
        assert!(first.memory_total_bytes.is_some_and(|n| n > 0));
        assert_eq!(sampler.sample("host-b").unwrap().environment, "host-b");
        assert!(sampler.sample("").is_err());
        std::thread::sleep(
            sysinfo::MINIMUM_CPU_UPDATE_INTERVAL + std::time::Duration::from_millis(30),
        );
        let second = sampler.sample("host-a").unwrap();
        assert!(second.cpu_percent.is_some());
        second.validate().unwrap();
    }
}
