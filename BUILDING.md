Building
========

With a rust toolchain installed, the testrunner can be built with:

```
cargo build --release
```


Building for portability (Linux)
--------------------------------

If a compiled copy of the testrunner will be distributed to your users, then
you may want to build the testrunner with a statically-linked musl-libc, instead
of a dynamically-linked glibc. This makes sure the testrunner will still work on
different linux distributions, with potentially different versions of glibc.

First, install the musl-libc target:

```
rustup target add x86_64-unknown-linux-musl
```

Afterwards, the testrunner can be built with:

```
cargo build --release --target x86_64-unknown-linux-musl
```


Building for size
-----------------

If the copies distributed to your users are hosted at the same location, such
as a central git server where every user has its own repository, then you may want
to build for a reduced binary size.
This can be achieved by also building the standard-library, instead of using a
prebuilt one.

First, install a rust-nightly toolchain and the necessary component(s):

```
rustup toolchain install nightly
rustup component add rust-src --toolchain nightly
```

Afterwards, the testrunner can be built with:

```
cargo +nightly build -Z build-std=std --release
```


Building for portability and size (Linux)
-----------------------------------------

The above commands can be combined to build for both portability and size.

First, install the necessary targets and component(s), if not installed:

```
rustup toolchain install nightly
rustup component add rust-src --toolchain nightly
rustup +nightly target add x86_64-unknown-linux-musl
```

Afterwards, the testrunner can be built with:

```
cargo +nightly build -Z build-std=std --release --target x86_64-unknown-linux-musl
```

