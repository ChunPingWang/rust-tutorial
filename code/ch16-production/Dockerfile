# 多階段建置：第一階段編譯，第二階段只放執行檔。
# 最終映像不含 Rust 工具鏈，約 30MB（distroless）而不是 2GB。

# ---------- 階段一：編譯 ----------
FROM rust:1.90-bookworm AS builder
WORKDIR /app

# 先只複製 Cargo.toml/Cargo.lock 並建一個假的 main，讓相依套件的編譯結果被 Docker 層快取。
# 之後只改程式碼時，不需要重新編譯所有相依。
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src/bin && echo 'fn main() {}' > src/main.rs \
    && echo 'fn main() {}' > src/bin/server.rs \
    && echo '' > src/lib.rs \
    && cargo build --release --bin server \
    && rm -rf src

# 再複製真正的程式碼編譯
COPY src ./src
RUN touch src/lib.rs src/bin/server.rs && cargo build --release --bin server

# ---------- 階段二：執行 ----------
# distroless：沒有 shell、沒有套件管理器，攻擊面最小。Rust 執行檔只需要 glibc。
FROM gcr.io/distroless/cc-debian12
COPY --from=builder /app/target/release/server /usr/local/bin/ledger-server
ENV LEDGER_ADDR=0.0.0.0:3000
ENV RUST_LOG=info
EXPOSE 3000
USER nonroot
ENTRYPOINT ["/usr/local/bin/ledger-server"]
