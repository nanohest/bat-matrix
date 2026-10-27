use std::error::Error;
use std::time::Duration;

use dbus::blocking::Connection;
use dbus::blocking::stdintf::org_freedesktop_dbus::Properties;
use serialport::SerialPort;

const MATRIX_PORTS: [&str; 2] = ["/dev/ttyACM0", "/dev/ttyACM1"];
const BATTERY_PATH: &str = "/org/freedesktop/UPower/devices/DisplayDevice";
const UPOWER_PATH: &str = "/org/freedesktop/UPower";

fn main() -> Result<(), Box<dyn Error>> {
    let connection = Connection::new_system()?;

    // Listen for battery percentage and AC power changes.
    connection.add_match_no_cb(&format!(
        "type='signal',sender='org.freedesktop.UPower',interface='org.freedesktop.DBus.Properties',member='PropertiesChanged',path='{BATTERY_PATH}'"
    ))?;
    connection.add_match_no_cb(&format!(
        "type='signal',sender='org.freedesktop.UPower',interface='org.freedesktop.DBus.Properties',member='PropertiesChanged',path='{UPOWER_PATH}'"
    ))?;

    loop {
        update_matrices(&connection)?;
        connection.process(Duration::from_secs(60))?;
    }
}

fn update_matrices(connection: &Connection) -> Result<(), Box<dyn Error>> {
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
    let brightness = if on_battery { 10 } else { 90 };
    let image = battery_image(percentage);

    for device in MATRIX_PORTS {
        let mut port = serialport::new(device, 115_200)
            .timeout(Duration::from_secs(2))
            .open()
            .map_err(|error| format!("Could not open {device}: {error}"))?;
        set_brightness(&mut *port, brightness)
            .map_err(|error| format!("Could not set brightness on {device}: {error}"))?;
        display_image(&mut *port, &image)
            .map_err(|error| format!("Could not draw battery on {device}: {error}"))?;
    }

    println!(
        "Battery: {percentage}% ({} power)",
        if on_battery { "battery" } else { "AC" }
    );
    Ok(())
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
