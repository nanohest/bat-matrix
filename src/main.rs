use std::error::Error;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use std::time::Instant;

use dbus::blocking::Connection;
use dbus::blocking::stdintf::org_freedesktop_dbus::Properties;
use serialport::SerialPort;

static DEBUG: AtomicBool = AtomicBool::new(false);
const MATRIX_PORTS: [&str; 2] = ["/dev/ttyACM0", "/dev/ttyACM1"];
const BATTERY_PATH: &str = "/org/freedesktop/UPower/devices/DisplayDevice";
const UPOWER_PATH: &str = "/org/freedesktop/UPower";
const HELP: &str = "\
USAGE: bat-matrix [OPTIONS]

Show battery level on your Framework LED Matrix module(s)

OPTIONS:
  -d, --device      Choose a single device to display battery status on.
                    Eg.: /dev/ttyACM0
                    Default: both of /dev/ttyACM0 and /dev/ttyACM1
  -b, --on-battery  Set custom brightness level, for when laptop runs on battery, as integer (percentage)
                    Default: 2
  -a, --on-ac       Set custom brightness level, for when laptop runs on AC power, as integer (percentage)
                    Default: 50
  -v, --verbose     Enable verbose output. Mostly usable for debugging.

Examples:
  Show battery level on device ttyACM0 only
    bat-matrix -d /dev/ttyACM0

  Set brightness when running on battery to 10%
    bat-matrix -b 10

  Set brightness when running on AC power to 90%
    bat-matrix -a 90
";

struct Args {
    device: Option<String>,
    on_battery: u8,
    on_ac: u8,
    verbose: bool,
}

macro_rules! debug {
    ($($arg:tt)*) => {
        if DEBUG.load(Ordering::Relaxed) {
            eprintln!($($arg)*);
        }
    };
}

fn parse_args() -> Result<Args, lexopt::Error> {
    use lexopt::prelude::*;

    let mut device = None;
    let mut on_battery = 2;
    let mut on_ac = 50;
    let mut verbose = false;
    let mut parser = lexopt::Parser::from_env();
    while let Some(arg) = parser.next()? {
        match arg {
            Short('d') | Long("device") => device = Some(parser.value()?.string()?),
            Short('b') | Long("on-battery") => on_battery = parser.value()?.parse()?,
            Short('a') | Long("on-ac") => on_ac = parser.value()?.parse()?,
            Short('v') | Long("verbose") => verbose = true,
            Short('h') | Long("help") => {
                print!("{}", HELP);
                std::process::exit(0);
            }
            _ => return Err(arg.unexpected()),
        }
    }
    Ok(Args {
        device,
        on_battery,
        on_ac,
        verbose,
    })
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = parse_args()?;
    let devices: Vec<String> = match args.device {
        Some(device) => vec![device],
        None => MATRIX_PORTS.iter().map(|s| s.to_string()).collect(),
    };
    let bat_brightness = args.on_battery;
    let ac_brightness = args.on_ac;
    DEBUG.store(args.verbose, Ordering::Relaxed);

    let connection = Connection::new_system()?;

    // Listen for battery percentage and AC power changes.
    connection.add_match_no_cb(&format!(
        "type='signal',sender='org.freedesktop.UPower',interface='org.freedesktop.DBus.Properties',member='PropertiesChanged',path='{BATTERY_PATH}'"
    ))?;
    connection.add_match_no_cb(&format!(
        "type='signal',sender='org.freedesktop.UPower',interface='org.freedesktop.DBus.Properties',member='PropertiesChanged',path='{UPOWER_PATH}'"
    ))?;

    const KEEPALIVE: Duration = Duration::from_secs(50);
    let mut next_ping = Instant::now() + KEEPALIVE;
    let mut got_signal = true; // draw once at startup
    let mut last_img: Option<(u8, [u8; 39])> = None;
    let mut last_pct: u8 = 0;
    let mut last_pass = Instant::now();

    loop {
        if DEBUG.load(Ordering::Relaxed) {
            let now = Instant::now();
            let elapsed = now.duration_since(last_pass).as_secs();
            last_pass = now;

            println!("Seconds since last DBus UPower signal: +{elapsed}s");
        }

        if got_signal {
            if let Err(e) = update_matrices(
                &connection,
                &mut last_img,
                &mut last_pct,
                &devices,
                bat_brightness,
                ac_brightness,
            ) {
                eprintln!("{e}");
            }
        } else if Instant::now() >= next_ping {
            for device in &devices {
                let mut port = serialport::new(device, 115_200)
                    .timeout(Duration::from_secs(2))
                    .open()
                    .map_err(|error| format!("Could not open {device}: {error}"))?;
                wake_up(&mut *port)
                    .map_err(|error| format!("Could not wake up {device}: {error}"))?;
            }
            next_ping = Instant::now() + KEEPALIVE;
        }
        got_signal = connection.process(next_ping.saturating_duration_since(Instant::now()))?;
    }
}

fn update_matrices(
    connection: &Connection,
    last_img: &mut Option<(u8, [u8; 39])>,
    last_pct: &mut u8,
    devices: &[String],
    bat_brightness: u8,
    ac_brightness: u8,
) -> Result<(), Box<dyn Error>> {
    let upower = connection.with_proxy(
        "org.freedesktop.UPower",
        UPOWER_PATH,
        Duration::from_secs(5),
    );
    let battery = connection.with_proxy(
        "org.freedesktop.UPower",
        BATTERY_PATH,
        Duration::from_secs(5),
    );

    let on_battery: bool = upower
        .get("org.freedesktop.UPower", "OnBattery")
        .map_err(|error| format!("Could not read UPower AC status: {error}"))?;
    let percentage: f64 = battery
        .get("org.freedesktop.UPower.Device", "Percentage")
        .map_err(|error| format!("Could not read UPower battery percentage: {error}"))?;
    let percentage = percentage.round().clamp(0.0, 100.0) as u8;
    // Brightness is given as percent from input args
    // Modules read a value from 0 to 255
    // Convert percentage to 0-255 scale
    let brightness_pct = if on_battery {
        bat_brightness
    } else {
        ac_brightness
    };
    let brightness_abs = (u16::from(brightness_pct.min(100)) * 255 / 100) as u8;
    let image = battery_image(percentage);

    // DEBUG: Write percentage if it has changed
    if *last_pct != percentage {
        *last_pct = percentage;
        debug!(
            "Battery: {percentage}% ({} power)",
            if on_battery { "battery" } else { "AC" }
        );
    }

    if *last_img == Some((brightness_abs, image)) {
        return Ok(());
    }

    for device in devices {
        let mut port = serialport::new(device, 115_200)
            .timeout(Duration::from_secs(2))
            .open()
            .map_err(|error| format!("Could not open {device}: {error}"))?;
        set_brightness(&mut *port, brightness_abs)
            .map_err(|error| format!("Could not set brightness on {device}: {error}"))?;
        display_image(&mut *port, &image)
            .map_err(|error| format!("Could not draw battery on {device}: {error}"))?;
    }

    *last_img = Some((brightness_abs, image));

    Ok(())
}

fn wake_up(port: &mut dyn SerialPort) -> Result<(), Box<dyn Error>> {
    send_command(port, 0x03, &[0])
}

fn set_brightness(port: &mut dyn SerialPort, brightness: u8) -> Result<(), Box<dyn Error>> {
    send_command(port, 0x00, &[brightness])
}

fn display_image(port: &mut dyn SerialPort, image: &[u8; 39]) -> Result<(), Box<dyn Error>> {
    send_command(port, 0x06, image)
}

fn send_command(
    port: &mut dyn SerialPort,
    command: u8,
    arguments: &[u8],
) -> Result<(), Box<dyn Error>> {
    let mut packet = [0; 42];
    packet[..3].copy_from_slice(&[0x32, 0xAC, command]);
    packet[3..3 + arguments.len()].copy_from_slice(arguments);
    port.write_all(&packet[..3 + arguments.len()])
        .map_err(Into::into)
}

fn battery_image(percentage: u8) -> [u8; 39] {
    let mut image = [0; 39];
    let mut set_pixel = |x: usize, y: usize| {
        let pixel = x + 9 * y;
        image[pixel / 8] |= 1 << (pixel % 8);
    };

    // Battery terminal and outer case.
    for x in 3..=5 {
        set_pixel(x, 4);
        set_pixel(x, 5);
    }
    for x in 1..=7 {
        set_pixel(x, 6);
        set_pixel(x, 28);
    }
    for y in 7..=27 {
        set_pixel(1, y);
        set_pixel(7, y);
    }

    // Fill the inside from the bottom, leaving the outline clear.
    let filled_rows = usize::from(percentage.min(100)) * 19 / 100;
    for y in (27 - filled_rows)..27 {
        for x in 2..=6 {
            set_pixel(x, y);
        }
    }

    image
}

#[cfg(test)]
mod tests {
    use super::battery_image;

    #[test]
    fn battery_fill_grows_with_percentage() {
        let empty = battery_image(0);
        let half = battery_image(50);
        let full = battery_image(100);

        assert!(empty.iter().zip(half).any(|(empty, half)| empty != &half));
        assert!(half.iter().zip(full).any(|(half, full)| half != &full));
        assert_ne!(empty, full);
    }

    #[test]
    fn battery_percentage_is_capped_at_full() {
        assert_eq!(battery_image(100), battery_image(255));
    }
}
