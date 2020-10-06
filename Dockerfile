FROM ubuntu:latest

ENV DEBIAN_FRONTEND noninteractive
ENV DEBCONF_NONINTERACTIVE_SEEN true

RUN echo "tzdata tzdata/Areas select Europe" > /tmp/preseed.txt; \
    echo "tzdata tzdata/Zones/Europe select Vienna" >> /tmp/preseed.txt; \
    debconf-set-selections /tmp/preseed.txt 

RUN apt update --yes && \
    apt install build-essential curl clang --yes

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs > rustup.sh && \
    chmod +x rustup.sh && \
    ./rustup.sh -y && \
    echo "export PATH=$PATH:/root/.cargo/bin" >> /root/.bashrc
