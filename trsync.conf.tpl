[server]
instances = algoo,bux
local_folder = /home/<your user>/Tracim
# Required only if you use the configure (or systray) application
trsync_manager_configure_bin_path = "/usr/local/bin/trsync-manager-configure"

[instance.algoo]
address = algoo.tracim.fr
username = bux
unsecure = false
workspaces_ids = 42,43

[instance.bux]
address = tracim.bux.fr
username = bux
unsecure = false
workspaces_ids = 24,23