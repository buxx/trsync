# Deployment

## Graphical usage

This documentation expose a possible solution to deploy TrSync for a graphical usage, on a Debian-like system.

### Binaries

Two binaries are required :

* `trsync-manager-systray`, see [README.md](../README.md) for compilation.
* `trsync-manager-configure`, see [github.com/buxx/trsync-manager-configure](https://github.com/buxx/trsync-manager-configure)

Place binaries on your system at :

* `/usr/bin/trsync-manager-systray`
* `/usr/bin/trsync-manager-configure`

### Configuration file

Each user which will use TrSync must have a configuration file at :

* `/home/<username>/.trsync.conf`

This file must contain :

```ini
[server]
instances =
local_folder = /home/<username>/Tracim
icons_path = </home/<username>/.local/share/icons if install for one or some user only, /usr/share/icons if install for all users>
```

At this step, TrSync can be started by executing `/usr/bin/trsync-manager-configure`.

## TrSync as Application

1. To permit start TrSync though graphical menus, create a .desktop file (see content below):
   * `/home/<username>/.local/share/applications` if install for one or some user only
   * `/usr/share/applications` if install for all users
2. Copy all .png files from `systray` folder to:
   * `/home/<username>/.local/share/icons/` if install for current user only
   * `/usr/share/icons` if install for all users.

This desktop file must contain :

```ini
[Desktop Entry]
Encoding=UTF-8
Version=<TrSync version>
Type=Application
Terminal=false
Exec=/usr/bin/trsync-manager-systray
Name=TrSync
Icon=</home/<username>/.local/share/icons/trsync.png if install for current user only, /usr/share/icons/trsync.png if install for all users>
```

## Auto startup

For concerned users, create following file :

* `/home/<username>/.config/autostart/trsync-manager-systray.desktop`

This file must contain :

```ini
[Desktop Entry]
Type=Application
Exec=/usr/bin/trsync-manager-systray
Hidden=false
NoDisplay=false
X-GNOME-Autostart-enabled=true
Name[fr_FR]=TrSync
Name=TrSync
Comment[fr_FR]=Synchronisation entre vos Tracim et vos dossiers
Comment=Synchronization between your Tracim and yours folders
```
