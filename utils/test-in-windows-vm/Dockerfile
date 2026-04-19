FROM archlinux:latest AS pacman-base

RUN --mount=type=cache,target=/var/cache/pacman/pkg \
    pacman -Syu --noconfirm && \
    pacman -S --noconfirm \
        openssh \
        qemu-full \
        libvirt \
        dnsmasq \
        iptables \
        openbsd-netcat \
        curl \
        rsync \
        pkgconf \
        git \
        base-devel \
        sudo \
        rubygems \
        libxslt \
        ruby \
        go

FROM pacman-base AS aur-builder

RUN useradd -m builder && \
    echo "builder ALL=(ALL) NOPASSWD: ALL" >> /etc/sudoers
RUN --mount=type=cache,target=/var/cache/pacman/pkg \
    sudo -u builder bash -c ' \
        echo "OPTIONS+=('"'"'!debug'"'"')" >> ~/.makepkg.conf && \
        cd /tmp && \
        git clone https://aur.archlinux.org/vagrant.git && \
        cd vagrant && \
        makepkg -s --noconfirm'

FROM pacman-base

COPY --from=aur-builder /tmp/vagrant/*.pkg.tar.zst /tmp/
RUN --mount=type=cache,target=/var/cache/pacman/pkg \
    pacman -U --noconfirm /tmp/*.pkg.tar.zst && \
    rm /tmp/*.pkg.tar.zst

WORKDIR /app
COPY --chmod=755 entrypoint-in-container.sh /app/
COPY Vagrantfile /app/

CMD ["/app/entrypoint-in-container.sh"]
