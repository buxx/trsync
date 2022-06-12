FROM ghcr.io/cross-rs/x86_64-unknown-linux-gnu:main

RUN yum groupinstall 'Development Tools' -y
RUN yum -y install epel-release && yum repolist
RUN yum --setopt=skip_missing_names_on_install=False install \
        openssl-devel \
        pkgconfig \
        gdk-pixbuf2-devel \
        pango \
        pango-devel \
        cairo-gobject-devel \
        atk-devel \
        webkitgtk3-devel \
        libappindicator-gtk3-devel \
        libsqlite3x-devel \
        -y
