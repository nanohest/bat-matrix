# Bat-Matrix

Bat-Matrix is a battery monitor, sitting on Led Matrix modules for Framework 16 laptops.

## Install as a user service

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

Check its status and logs with:

```sh
systemctl --user status bat-matrix.service
journalctl --user -u bat-matrix.service -f
```

The user running the service must have permission to access both LED matrix
serial devices. If you change group membership to grant access, log out and
back in before starting the service again.
