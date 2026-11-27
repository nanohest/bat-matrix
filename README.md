# Bat-Matrix

Bat-Matrix is a battery monitor, sitting on Led Matrix modules for Framework 16 laptops.

## Install

Install the latest release and enable it as a systemd user service:

```sh
curl -fsSL https://raw.githubusercontent.com/nanohest/bat-matrix/main/install.sh | sh
```

Options are passed after `sh -s --`, for example:

```sh
curl -fsSL https://raw.githubusercontent.com/nanohest/bat-matrix/main/install.sh \
  | sh -s -- --bin-dir /usr/local/bin --no-service
```

- `--bin-dir DIR` installs the binary into `DIR` instead of `~/.local/bin`.
  `sudo` is used if `DIR` is not writable.
- `--no-service` skips installing and enabling the systemd user service.

If [slsa-verifier](https://github.com/slsa-framework/slsa-verifier) is
installed, the script verifies the binary against the release's SLSA
provenance and refuses to install it if verification fails. Without it, the
script warns and installs the binary unverified.

## Build from source

Build the binary and install it at the path used by the service:

```sh
cargo build --release
install -Dm755 target/release/bat-matrix ~/.local/bin/bat-matrix
```

Install and enable the systemd user unit:

```sh
install -Dm644 systemd/bat-matrix.service \
  ~/.config/systemd/user/bat-matrix.service
systemctl --user daemon-reload
systemctl --user enable --now bat-matrix.service
```

## Managing the service

Check its status and logs with:

```sh
systemctl --user status bat-matrix.service
journalctl --user -u bat-matrix.service -f
```

The user running the service must have permission to access both LED matrix
serial devices. If you change group membership to grant access, log out and
back in before starting the service again.

## Usage

```console
$ bat-matrix -h
USAGE: batmon [OPTIONS]

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
    batmon -d /dev/ttyACM0

  Set brightness when running on battery to 10%
    batmon -b 10

  Set brightness when running on AC power to 90%
    batmon -a 90
```
