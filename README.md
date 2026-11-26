# Battery Monitor

## Install as a user service

Build the binary and install it at the path used by the service:

```sh
cargo build --release
install -Dm755 target/release/battery-monitor ~/.local/bin/battery-monitor
```

Install and enable the systemd user unit:

```sh
install -Dm644 systemd/battery-monitor.service \
  ~/.config/systemd/user/battery-monitor.service
systemctl --user daemon-reload
systemctl --user enable --now battery-monitor.service
```

Check its status and logs with:

```sh
systemctl --user status battery-monitor.service
journalctl --user -u battery-monitor.service -f
```

The user running the service must have permission to access both LED matrix
serial devices. If you change group membership to grant access, log out and
back in before starting the service again.
