[main]
bin = ./target/debug/trsync
log_to = ./{instance_name}_{workspace_name}.log
ask_password_ids = password1,
instance_names = tracimA.local,tracimB.local

[tracimA.local]
domain = tracimA.local
username = username_to_use
password_id = password1
workspace_names = MyDocuments,

[tracimA.local::MyDocuments]
folder_path = /home/my_username/Documents
remote_id = 42

[tracimB.local]
domain = tracimB.local
username = username_to_use
password_id = password1
workspace_names = Client1,Client2

[tracimA.local::Client1]
folder_path = /home/my_username/Client1
remote_id = 2

[tracimA.local::Client2]
folder_path = /home/my_username/Client2
remote_id = 36
