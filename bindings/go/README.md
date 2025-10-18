# termprobe for Go

Control, inspect, test, and record terminal apps from Go. The binding runs the Rust engine in your process and does not require the CLI.

## Install

Use Go 1.26 or newer and select a published Go module release:

```sh
go get github.com/microsoft/termprobe/bindings/go@v0.1.1
```

The repository tag is `bindings/go/v<version>`.
Released modules contain prebuilt native engines. Users need no Rust compiler,
Zig, C compiler, CLI, or separate installer.
Source checkouts do not contain these binaries; see [Build and use](../../crates/termprobe-ffi/README.md#build-and-use).

| OS | Go architectures | Native runtime |
| --- | --- | --- |
| Linux | `amd64`, `arm64` | glibc 2.28+ or musl 1.2+ |
| macOS | `amd64`, `arm64` | macOS system libraries |
| Windows | `amd64` | Windows system libraries |

Go downloads all platform archives with the module.
Each executable embeds compressed archives for its target OS and architecture.
Linux executables contain both libc variants and select the one that matches the running process.
Other platforms contain one variant.

On first use, the binding extracts the selected engine into the user cache:
`termprobe/native/<sha256>/`.
Later processes verify and reuse the same file.
The engine stays loaded for the process lifetime.
There are no runtime downloads.

The cache must permit file creation, hard links, and native library loading.
For a read-only or restricted environment, use an existing installation as described below.

### Linux and cross-compilation

PureGo avoids cgo, but it still uses the system dynamic loader and libc.
Embedding a library does not make the executable statically linked.
Native glibc builds select the GNU engine. Normal native Alpine builds select the musl engine.
Neither needs a custom Go build tag.
Cgo-enabled applications can require a newer libc than the bundled engine.

Cross-compile for glibc Linux with:

```sh
CGO_ENABLED=0 GOOS=linux GOARCH=amd64 go build .
```

For musl, also select the destination interpreter:

```sh
CGO_ENABLED=0 GOOS=linux GOARCH=amd64 \
  go build -ldflags '-I /lib/ld-musl-x86_64.so.1' .
```

