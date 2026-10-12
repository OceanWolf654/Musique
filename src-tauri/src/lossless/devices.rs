use cpal::traits::{DeviceTrait, HostTrait};
use super::types::AudioDeviceFormatInfo;

const AUDIOPHILE_SAMPLE_RATES: [u32; 6] = [44100, 48000, 88200, 96000, 176400, 192000];

/// Enumerate audio output devices and their supported sample rate ranges.
pub fn list_output_devices() -> Vec<AudioDeviceFormatInfo> {
    let host = cpal::default_host();
    let default_name = host
        .default_output_device()
        .and_then(|d| d.name().ok());

    let mut result = Vec::new();
    let devices = match host.output_devices() {
        Ok(devs) => devs,
        Err(e) => {
            log::warn!("Failed to query audio output devices: {e}");
            return result;
        }
    };

    for device in devices {
        let name = match device.name() {
            Ok(n) => n,
            Err(_) => continue,
        };

        let is_default = default_name.as_ref().map(|d| d == &name).unwrap_or(false);
        let default_config = device.default_output_config().ok();
        let default_sample_rate = default_config
            .as_ref()
            .map(|c| c.sample_rate().0)
            .unwrap_or(48000);
        let default_channels = default_config
            .as_ref()
            .map(|c| c.channels())
            .unwrap_or(2);

        let mut supported_rates = Vec::new();
        if let Ok(configs_iter) = device.supported_output_configs() {
            let ranges: Vec<_> = configs_iter.collect();
            for rate in AUDIOPHILE_SAMPLE_RATES {
                // Check if any supported range covers this rate
                for range in &ranges {
                    if rate >= range.min_sample_rate().0 && rate <= range.max_sample_rate().0 {
                        if !supported_rates.contains(&rate) {
                            supported_rates.push(rate);
                        }
                    }
                }
            }
        }
        if supported_rates.is_empty() {
            supported_rates.push(default_sample_rate);
        }
        supported_rates.sort_unstable();

        result.push(AudioDeviceFormatInfo {
            name,
            is_default,
            default_sample_rate,
            supported_sample_rates: supported_rates,
            default_channels,
            supports_exclusive: cfg!(target_os = "windows"),
        });
    }

    result
}

/// Find default device format info.
pub fn get_default_device_info() -> Option<AudioDeviceFormatInfo> {
    list_output_devices().into_iter().find(|d| d.is_default)
}
